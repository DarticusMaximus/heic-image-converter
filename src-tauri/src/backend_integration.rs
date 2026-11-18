//! Headless integration tests for the backend: folder scanning now, with
//! thumbnail and batch coverage added alongside it.
//!
//! Nothing here needs a Tauri `AppHandle`, a window, or a network. Every write
//! goes to a `tempfile::tempdir()`; committed fixtures under `tests/fixtures/`
//! are copied into those temp dirs and never modified.

use crate::convert::{ConvertItem, FileOutcome, ProgressEvent, convert_all};
use crate::fixture_corpus::fixture_path;
use crate::formats::{ConvertSettings, InputKind, OutputFormat, Quality};
use crate::scan;
use base64::Engine;
use image::ExtendedColorType;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// A path as the `String` the scan and convert entry points take.
fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Drop paths in the shape the frontend sends them.
fn drop_paths(paths: &[&Path]) -> Vec<String> {
    paths.iter().map(|path| path_string(path)).collect()
}

/// A scan result's `relative` as a `Path`, panicking with the file that has
/// none instead of silently skipping the assertion.
fn relative_of(file: &scan::SourceFile) -> PathBuf {
    file.relative
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{} came back without a relative path", file.path))
}

fn ensure_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|err| panic!("could not create {}: {err}", parent.display()));
    }
}

/// Copy a committed fixture to `destination`, creating parent folders. Only
/// reads the corpus, so the committed fixtures stay untouched.
fn copy_fixture(fixture: &str, destination: &Path) -> PathBuf {
    let source = fixture_path(fixture);
    assert!(
        source.is_file(),
        "committed fixture {} is missing",
        source.display()
    );
    ensure_parent(destination);
    std::fs::copy(&source, destination).unwrap_or_else(|err| {
        panic!(
            "could not copy {} to {}: {err}",
            source.display(),
            destination.display()
        )
    });
    destination.to_path_buf()
}

/// Write `bytes` to `path`, creating parent folders.
fn write_file(path: &Path, bytes: &[u8]) -> PathBuf {
    ensure_parent(path);
    std::fs::write(path, bytes)
        .unwrap_or_else(|err| panic!("could not write {}: {err}", path.display()));
    path.to_path_buf()
}

/// Deterministic RGB pixels: a gradient, so every pixel is distinct without
/// caring about exact colours.
fn sample_rgb(width: u32, height: u32) -> image::RgbImage {
    image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
    })
}

/// Write a plain PNG of exactly `width`x`height`.
fn write_png(path: &Path, width: u32, height: u32) -> PathBuf {
    ensure_parent(path);
    sample_rgb(width, height)
        .save(path)
        .unwrap_or_else(|err| panic!("could not write {}: {err}", path.display()));
    path.to_path_buf()
}

/// Write a plain JPEG of exactly `width`x`height`.
fn write_jpeg(path: &Path, width: u32, height: u32) -> PathBuf {
    ensure_parent(path);
    sample_rgb(width, height)
        .save(path)
        .unwrap_or_else(|err| panic!("could not write {}: {err}", path.display()));
    path.to_path_buf()
}

/// Write a single-frame GIF of exactly `width`x`height`.
fn write_gif(path: &Path, width: u32, height: u32) -> PathBuf {
    ensure_parent(path);
    let rgba = image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255])
    });
    let file = std::fs::File::create(path)
        .unwrap_or_else(|err| panic!("could not create {}: {err}", path.display()));
    image::codecs::gif::GifEncoder::new(std::io::BufWriter::new(file))
        .encode(
            rgba.as_raw(),
            rgba.width(),
            rgba.height(),
            ExtendedColorType::Rgba8,
        )
        .unwrap_or_else(|err| panic!("could not write {}: {err}", path.display()));
    path.to_path_buf()
}

/// Conversion settings for the batch tests; resize stays off unless a test
/// needs it.
fn settings(
    format: OutputFormat,
    quality: Quality,
    output_dir: Option<&Path>,
    overwrite: bool,
) -> ConvertSettings {
    ConvertSettings {
        format,
        quality,
        max_dimension: None,
        output_dir: output_dir.map(path_string),
        overwrite,
    }
}

/// A queue entry in the shape the UI sends: source path plus the dropped folder
/// and the path relative to it.
fn convert_item(source: &Path, root: Option<&Path>, relative: Option<&str>) -> ConvertItem {
    ConvertItem {
        path: path_string(source),
        root: root.map(path_string),
        relative: relative.map(str::to_string),
    }
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

#[test]
fn scan_walks_recursively_with_roots_and_relatives() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("root");

    write_png(&root.join("a.png"), 8, 6);
    write_jpeg(&root.join("sub").join("b.jpg"), 10, 8);
    write_gif(&root.join("sub").join("deep").join("c.gif"), 6, 5);

    let canonical_root = root.canonicalize().expect("canonical dropped folder");
    let result = scan::scan(&drop_paths(&[&root]));

    assert!(
        result.skipped.is_empty(),
        "a clean folder must not skip anything, got {:?}",
        result
            .skipped
            .iter()
            .map(|skipped| (&skipped.path, &skipped.reason))
            .collect::<Vec<_>>()
    );
    assert!(!result.truncated, "three files must not hit the scan cap");
    assert_eq!(
        result.files.len(),
        3,
        "expected a.png, sub/b.jpg, sub/deep/c.gif, got {:?}",
        result
            .files
            .iter()
            .map(|file| &file.path)
            .collect::<Vec<_>>()
    );

    let expected: [(PathBuf, &str, u32, u32); 3] = [
        (PathBuf::from("a.png"), "png", 8, 6),
        (PathBuf::from("sub").join("b.jpg"), "jpeg", 10, 8),
        (PathBuf::from("sub").join("deep").join("c.gif"), "gif", 6, 5),
    ];

    let relative: Vec<PathBuf> = result.files.iter().map(relative_of).collect();
    let expected_relative: Vec<PathBuf> = expected
        .iter()
        .map(|(relative, ..)| relative.clone())
        .collect();
    assert_eq!(
        relative, expected_relative,
        "files must come back in path order with paths relative to the dropped folder"
    );

    let paths: Vec<&str> = result.files.iter().map(|file| file.path.as_str()).collect();
    let sorted = {
        let mut sorted = paths.clone();
        sorted.sort_unstable();
        sorted
    };
    assert_eq!(paths, sorted, "files must come back in path order");

    for (file, (relative, kind, width, height)) in result.files.iter().zip(&expected) {
        let canonical = canonical_root.to_string_lossy();
        assert_eq!(
            file.root.as_deref(),
            Some(canonical.as_ref()),
            "{} must be rooted at the canonical dropped folder",
            file.path
        );
        assert_eq!(
            Path::new(&file.path),
            canonical_root.join(relative),
            "{} is not the file under {}",
            file.path,
            relative.display()
        );
        assert_eq!(file.kind, *kind, "{}", file.path);
        assert!(file.bytes > 0, "{} has no size on disk", file.path);
        assert_eq!(
            (file.width, file.height),
            (Some(*width), Some(*height)),
            "{} must report {}x{}",
            file.path,
            width,
            height
        );
    }
}

#[test]
fn scan_skips_hidden_and_system_entries() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("root");

    write_png(&root.join("keep.png"), 8, 6);
    write_png(&root.join(".dot.png"), 8, 6);
    write_png(&root.join(".hidden").join("inner.png"), 9, 7);
    write_file(&root.join("Thumbs.db"), b"system junk");
    write_file(&root.join("desktop.ini"), b"[.ShellClassInfo]");

    let result = scan::scan(&drop_paths(&[&root]));

    assert!(
        result.skipped.is_empty(),
        "hidden and system entries are ignored without noise, got {:?}",
        result
            .skipped
            .iter()
            .map(|skipped| (&skipped.path, &skipped.reason))
            .collect::<Vec<_>>()
    );
    assert!(!result.truncated);
    assert_eq!(
        result.files.len(),
        1,
        "only keep.png may be listed, got {:?}",
        result
            .files
            .iter()
            .map(|file| &file.path)
            .collect::<Vec<_>>()
    );
    let only = &result.files[0];
    assert_eq!(only.name, "keep.png");
    assert_eq!(relative_of(only), PathBuf::from("keep.png"));

    // The dropped folder itself is exempt from the hidden check: dropping
    // `.hidden` directly still finds what is inside it.
    let hidden_root = root.join(".hidden");
    let canonical_hidden = hidden_root.canonicalize().expect("canonical hidden root");
    let hidden_result = scan::scan(&drop_paths(&[&hidden_root]));

    assert_eq!(
        hidden_result.files.len(),
        1,
        "scanning the dropped .hidden folder must find inner.png"
    );
    let inner = &hidden_result.files[0];
    assert_eq!(inner.name, "inner.png");
    assert_eq!(relative_of(inner), PathBuf::from("inner.png"));
    assert_eq!(
        inner.root,
        Some(canonical_hidden.to_string_lossy().into_owned()),
        "inner.png must be rooted at the dropped .hidden folder"
    );
}

#[test]
fn scan_ignores_unsupported_files_inside_folders() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("root");

    write_png(&root.join("keep.png"), 8, 6);
    write_file(&root.join("notes.txt"), b"not an image");

    let result = scan::scan(&drop_paths(&[&root]));

    assert_eq!(
        result.files.len(),
        1,
        "only keep.png may be listed, got {:?}",
        result
            .files
            .iter()
            .map(|file| &file.path)
            .collect::<Vec<_>>()
    );
    assert_eq!(result.files[0].name, "keep.png");
    assert!(
        result.skipped.is_empty(),
        "unsupported files inside a dropped folder are ignored without noise, got {:?}",
        result
            .skipped
            .iter()
            .map(|skipped| (&skipped.path, &skipped.reason))
            .collect::<Vec<_>>()
    );
    assert!(!result.truncated);
}

#[test]
fn scan_reports_explicitly_dropped_files_with_reasons() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("root");

    let notes = write_file(&root.join("notes.txt"), b"not an image");
    let missing = temp.path().join("missing.png");

    let result = scan::scan(&drop_paths(&[&notes, &missing]));

    assert!(
        result.files.is_empty(),
        "neither an unsupported file nor a missing path yields a file, got {:?}",
        result
            .files
            .iter()
            .map(|file| &file.path)
            .collect::<Vec<_>>()
    );
    assert!(!result.truncated);
    assert_eq!(
        result.skipped.len(),
        2,
        "both dropped paths must be reported, got {:?}",
        result
            .skipped
            .iter()
            .map(|skipped| (&skipped.path, &skipped.reason))
            .collect::<Vec<_>>()
    );
    assert_eq!(result.skipped[0].path, path_string(&notes));
    assert_eq!(result.skipped[1].path, path_string(&missing));

    let reasons: Vec<&str> = result
        .skipped
        .iter()
        .map(|skipped| skipped.reason.as_str())
        .collect();
    for (skipped, reason) in result.skipped.iter().zip(&reasons) {
        assert!(
            !reason.trim().is_empty(),
            "{} was skipped with an empty reason",
            skipped.path
        );
    }
    assert_ne!(
        reasons[0], reasons[1],
        "an unsupported file and a missing path must explain themselves differently"
    );
}

#[test]
fn scan_deduplicates_canonical_paths() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("root");
    let file = write_png(&root.join("a.png"), 8, 6);

    let canonical_root = root.canonicalize().expect("canonical dropped folder");
    let canonical_file = file.canonicalize().expect("canonical dropped file");
    let indirect = root.join("..").join("root").join("a.png");

    // The same file arrives through the folder walk, twice as an explicit file,
    // and once by a path that only canonicalizes to it.
    let result = scan::scan(&drop_paths(&[&root, &file, &file, &indirect]));

    assert!(
        result.skipped.is_empty(),
        "every dropped path resolves, so nothing may be skipped, got {:?}",
        result
            .skipped
            .iter()
            .map(|skipped| (&skipped.path, &skipped.reason))
            .collect::<Vec<_>>()
    );
    assert!(!result.truncated);
    assert_eq!(
        result.files.len(),
        1,
        "one canonical file must be listed once even when dropped four ways, got {:?}",
        result
            .files
            .iter()
            .map(|file| &file.path)
            .collect::<Vec<_>>()
    );
    let only = &result.files[0];
    assert_eq!(only.name, "a.png");
    assert_eq!(
        Path::new(&only.path),
        canonical_file,
        "the deduplicated entry must keep the canonical path"
    );
    assert_eq!(
        only.root,
        Some(canonical_root.to_string_lossy().into_owned()),
        "the folder drop came first, so it owns the file's root"
    );

    // A folder and one of its subfolders overlap: the nested files must still
    // come back once each, rooted at the folder that reached them first.
    write_png(&root.join("sub").join("inner.png"), 7, 5);
    let sub = root.join("sub");
    let canonical_sub = sub.canonicalize().expect("canonical dropped subfolder");

    let nested = scan::scan(&drop_paths(&[&root, &sub]));
    assert_eq!(
        nested.files.len(),
        2,
        "a.png and sub/inner.png must each appear once, got {:?}",
        nested
            .files
            .iter()
            .map(|file| &file.path)
            .collect::<Vec<_>>()
    );
    let nested_relative: Vec<PathBuf> = nested.files.iter().map(relative_of).collect();
    assert_eq!(
        nested_relative,
        vec![
            PathBuf::from("a.png"),
            PathBuf::from("sub").join("inner.png")
        ],
        "the nested file must not be repeated under the subfolder drop"
    );
    for file in &nested.files {
        assert_eq!(
            file.root.as_deref(),
            Some(canonical_root.to_string_lossy().as_ref()),
            "{} was reached through the root drop first, so the root owns it",
            file.path
        );
    }

    let reversed = scan::scan(&drop_paths(&[&sub, &root]));
    assert_eq!(
        reversed.files.len(),
        2,
        "reversing the drop order must not duplicate the nested file, got {:?}",
        reversed
            .files
            .iter()
            .map(|file| &file.path)
            .collect::<Vec<_>>()
    );
    let inner = reversed
        .files
        .iter()
        .find(|file| file.name == "inner.png")
        .unwrap_or_else(|| panic!("inner.png went missing when the drops were reversed"));
    assert_eq!(
        inner.root,
        Some(canonical_sub.to_string_lossy().into_owned()),
        "the first drop wins, so the subfolder owns inner.png"
    );
    assert_eq!(
        relative_of(inner),
        PathBuf::from("inner.png"),
        "inner.png is only a top-level file relative to the subfolder drop"
    );
}

#[test]
fn scan_keeps_mixed_roots_separate_and_ordered() {
    let temp = tempfile::tempdir().expect("tempdir");
    let alpha = temp.path().join("alpha");
    let bravo = temp.path().join("bravo");

    write_png(&alpha.join("one.png"), 8, 6);
    write_jpeg(&alpha.join("sub").join("two.jpg"), 10, 8);
    write_png(&bravo.join("three.png"), 9, 7);
    write_gif(&bravo.join("sub").join("deep").join("four.gif"), 6, 5);

    let canonical_alpha = alpha.canonicalize().expect("canonical alpha root");
    let canonical_bravo = bravo.canonicalize().expect("canonical bravo root");
    let alpha_root = canonical_alpha.to_string_lossy().into_owned();
    let bravo_root = canonical_bravo.to_string_lossy().into_owned();

    let forward = scan::scan(&drop_paths(&[&alpha, &bravo]));
    let reversed = scan::scan(&drop_paths(&[&bravo, &alpha]));

    fn layout(result: &scan::ScanResult) -> Vec<(String, PathBuf)> {
        result
            .files
            .iter()
            .map(|file| {
                let root = file
                    .root
                    .clone()
                    .unwrap_or_else(|| panic!("{} came back without a root", file.path));
                (root, relative_of(file))
            })
            .collect()
    }

    // Root-then-path order, with each relative path measured from its own root.
    let expected = vec![
        (alpha_root.clone(), PathBuf::from("one.png")),
        (alpha_root.clone(), PathBuf::from("sub").join("two.jpg")),
        (
            bravo_root.clone(),
            PathBuf::from("sub").join("deep").join("four.gif"),
        ),
        (bravo_root.clone(), PathBuf::from("three.png")),
    ];

    assert_eq!(
        layout(&forward),
        expected,
        "two dropped folders must be grouped by root, then ordered by path"
    );
    assert_eq!(
        layout(&reversed),
        expected,
        "reversing the drop order must not change the root-then-path order"
    );

    for (result, label) in [(&forward, "forward"), (&reversed, "reversed")] {
        assert!(result.skipped.is_empty(), "{label}: nothing may be skipped");
        assert!(!result.truncated, "{label}: four files are under any cap");
        for file in &result.files {
            let root = file
                .root
                .as_deref()
                .unwrap_or_else(|| panic!("{label}: {} came back without a root", file.path));
            assert!(
                root == alpha_root || root == bravo_root,
                "{label}: {} has root {root}, which is neither dropped folder",
                file.path
            );
            assert_eq!(
                Path::new(&file.path),
                Path::new(root).join(relative_of(file)),
                "{label}: {} must be its own root plus its relative path",
                file.path
            );
        }
    }
}

#[test]
fn scan_truncates_at_the_limit() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("root");
    for name in ["a.png", "b.png", "c.png"] {
        write_png(&root.join(name), 8, 6);
    }
    let paths = drop_paths(&[&root]);

    let capped = crate::scan::scan_with_limit(&paths, 2);
    assert!(
        capped.skipped.is_empty(),
        "a clean folder must not skip anything, got {:?}",
        capped
            .skipped
            .iter()
            .map(|skipped| (&skipped.path, &skipped.reason))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        capped
            .files
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>(),
        vec!["a.png", "b.png"],
        "the cap must keep the path-order prefix"
    );
    assert!(
        capped.truncated,
        "two of three files means the scan was truncated"
    );

    let generous = crate::scan::scan_with_limit(&paths, 100);
    assert_eq!(
        generous.files.len(),
        3,
        "a generous cap must list every file, got {:?}",
        generous
            .files
            .iter()
            .map(|file| &file.path)
            .collect::<Vec<_>>()
    );
    assert!(
        !generous.truncated,
        "a cap above the file count must not report truncation"
    );

    let uncapped = scan::scan(&paths);
    assert_eq!(
        uncapped.files.len(),
        3,
        "three files are far below MAX_FILES"
    );
    assert!(
        !uncapped.truncated,
        "the production cap must not truncate three files"
    );
}

#[test]
fn scan_reports_post_rotation_dimensions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("root");

    copy_fixture("orientation-6.jpg", &root.join("rotated.jpg"));
    copy_fixture("photo.heic", &root.join("still.heic"));

    let result = scan::scan(&drop_paths(&[&root]));

    assert!(
        result.skipped.is_empty(),
        "both copies are supported images, got {:?}",
        result
            .skipped
            .iter()
            .map(|skipped| (&skipped.path, &skipped.reason))
            .collect::<Vec<_>>()
    );
    assert!(!result.truncated);
    assert_eq!(
        result.files.len(),
        2,
        "rotated.jpg and still.heic must both be listed, got {:?}",
        result
            .files
            .iter()
            .map(|file| &file.path)
            .collect::<Vec<_>>()
    );

    // EXIF orientation 6 is stored 40x30 but displays 30x40; the scan must
    // report the upright dimensions the conversion will produce.
    let expected: [(&str, &str, u32, u32); 2] = [
        ("rotated.jpg", "jpeg", 30, 40),
        ("still.heic", "heic", 40, 30),
    ];
    for (name, kind, width, height) in expected {
        let file = result
            .files
            .iter()
            .find(|file| file.name == name)
            .unwrap_or_else(|| {
                panic!(
                    "{name} is missing from {:?}",
                    result
                        .files
                        .iter()
                        .map(|file| &file.path)
                        .collect::<Vec<_>>()
                )
            });
        assert_eq!(file.kind, kind, "{name}");
        assert_eq!(
            (file.width, file.height),
            (Some(width), Some(height)),
            "{name} must report post-rotation dimensions {width}x{height}"
        );
        assert_eq!(relative_of(file), PathBuf::from(name));
    }
}

// ---------------------------------------------------------------------------
// Thumbnails
// ---------------------------------------------------------------------------

/// The JPEG pixels inside a thumbnail data URL. The prefix is asserted here so
/// a regression that returns PNG or raw bytes cannot pass a pixel-only check.
fn decode_thumbnail(data_url: &str) -> image::DynamicImage {
    const PREFIX: &str = "data:image/jpeg;base64,";
    let payload = data_url.strip_prefix(PREFIX).unwrap_or_else(|| {
        panic!(
            "thumbnail is not a JPEG data URL; got {:?}",
            &data_url[..data_url.len().min(64)]
        )
    });
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .unwrap_or_else(|err| panic!("thumbnail payload is not valid base64: {err}"));
    assert!(!bytes.is_empty(), "thumbnail payload decoded to zero bytes");
    assert!(
        bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "thumbnail payload does not start with the JPEG magic bytes: {:02x?}",
        &bytes[..bytes.len().min(4)]
    );
    assert!(
        bytes.ends_with(&[0xFF, 0xD9]),
        "thumbnail payload does not end with the JPEG end-of-image marker"
    );
    image::load_from_memory(&bytes)
        .unwrap_or_else(|err| panic!("thumbnail payload does not decode as an image: {err}"))
}

/// A thumbnail's longest edge, which is the knob `size` controls.
fn longest_edge(image: &image::DynamicImage) -> u32 {
    image.width().max(image.height())
}

#[test]
fn thumbnail_returns_a_jpeg_data_url() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source = write_png(&temp.path().join("source.png"), 400, 300);

    let thumbnail = scan::thumbnail(&path_string(&source), 128).expect("thumbnail a plain PNG");
    assert!(
        thumbnail.starts_with("data:image/jpeg;base64,"),
        "thumbnail must be a JPEG data URL, got {:?}",
        &thumbnail[..thumbnail.len().min(64)]
    );

    let image = decode_thumbnail(&thumbnail);
    let (width, height) = (image.width(), image.height());
    assert!(
        (i64::from(longest_edge(&image)) - 128).abs() <= 1,
        "400x300 at size 128 must have a longest edge of 128 +/- 1, got {width}x{height}"
    );

    // Triangle resampling preserves the aspect ratio, so the short edge lands
    // on 300 * 128 / 400 = 96.
    let expected_short = ((300.0_f64 * 128.0) / 400.0).round() as i64;
    let short = i64::from(width.min(height));
    assert!(
        (short - expected_short).abs() <= 1,
        "aspect ratio must be preserved: 400x300 at size 128 should be about {expected_short} on the short edge, got {width}x{height}"
    );

    let ratio = f64::from(width) / f64::from(height);
    let source_ratio = 400.0 / 300.0;
    assert!(
        (ratio - source_ratio).abs() < 0.05,
        "aspect ratio changed from {source_ratio:.4} to {ratio:.4} ({width}x{height})"
    );
}

#[test]
fn thumbnail_never_upscales() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source = write_png(&temp.path().join("small.png"), 40, 30);

    let thumbnail = scan::thumbnail(&path_string(&source), 128).expect("thumbnail a small PNG");
    let image = decode_thumbnail(&thumbnail);

    assert_eq!(
        (image.width(), image.height()),
        (40, 30),
        "a 40x30 image is already under size 128, so the thumbnail must keep 40x30 instead of upscaling"
    );
}

#[test]
fn thumbnail_clamps_requested_edge() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source = write_png(&temp.path().join("large.png"), 800, 600);

    // The requested edge is clamped to 32..=512, so 1 becomes 32 and 10_000
    // becomes 512. Both extremes must honour the clamp, not the request.
    for (requested, expected_edge) in [(1_u32, 32_u32), (10_000, 512)] {
        let thumbnail = scan::thumbnail(&path_string(&source), requested)
            .unwrap_or_else(|err| panic!("thumbnail at size {requested} failed: {err}"));
        let image = decode_thumbnail(&thumbnail);
        assert_eq!(
            longest_edge(&image),
            expected_edge,
            "800x600 at requested size {requested} must be clamped to a longest edge of {expected_edge}, got {}x{}",
            image.width(),
            image.height()
        );
    }
}

#[test]
fn thumbnail_composites_alpha_onto_white() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source = copy_fixture("alpha.png", &temp.path().join("alpha.png"));

    let thumbnail = scan::thumbnail(&path_string(&source), 128).expect("thumbnail alpha.png");
    let image = decode_thumbnail(&thumbnail);
    let (width, height) = (image.width(), image.height());
    assert_eq!(
        (width, height),
        (40, 30),
        "alpha.png is 40x30, which size 128 must not downscale"
    );

    // The bottom-right quadrant of alpha.png is fully transparent. Sample
    // inside that quadrant (two pixels in from the edge, so JPEG ringing at
    // the quadrant border and the image border is out of play): white proves
    // the alpha was composited onto white rather than dropped to black.
    let rgb = image.to_rgb8();
    let inset = (width.saturating_sub(3), height.saturating_sub(3));
    let corner = (width - 1, height - 1);
    for (x, y) in [inset, corner] {
        let pixel = rgb.get_pixel(x, y).0;
        assert!(
            pixel.iter().all(|channel| *channel >= 232),
            "transparent pixels must composite onto white: ({x},{y}) decoded as {pixel:?}, but every channel must be >= 232"
        );
    }
}

#[test]
fn thumbnail_handles_heic_input() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source = copy_fixture("photo.heic", &temp.path().join("still.heic"));

    let thumbnail = scan::thumbnail(&path_string(&source), 128).expect("thumbnail a HEIC file");
    assert!(
        thumbnail.starts_with("data:image/jpeg;base64,"),
        "thumbnail must be a JPEG data URL, got {:?}",
        &thumbnail[..thumbnail.len().min(64)]
    );

    let image = decode_thumbnail(&thumbnail);
    assert_eq!(
        (image.width(), image.height()),
        (40, 30),
        "photo.heic is 40x30, so its thumbnail must decode to 40x30"
    );
}

#[test]
fn thumbnail_rejects_unsupported_and_corrupt_inputs() {
    let temp = tempfile::tempdir().expect("tempdir");

    let notes = write_file(&temp.path().join("notes.txt"), b"not an image");
    let error =
        scan::thumbnail(&path_string(&notes), 128).expect_err("a .txt file has no thumbnail");
    assert!(
        error.contains("unsupported format"),
        "an unsupported extension must be rejected with \"unsupported format\", got {error:?}"
    );

    let corrupt = copy_fixture("garbage.png", &temp.path().join("garbage.png"));
    let error = scan::thumbnail(&path_string(&corrupt), 128)
        .expect_err("a corrupt PNG must not produce a thumbnail");
    assert!(
        !error.trim().is_empty(),
        "a corrupt image must fail with a message worth showing the user, got {error:?}"
    );
}

// ---------------------------------------------------------------------------
// Batch conversion
// ---------------------------------------------------------------------------

/// A file's length on disk, panicking with its path instead of defaulting.
fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path)
        .unwrap_or_else(|err| panic!("could not stat {}: {err}", path.display()))
        .len()
}

#[test]
fn batch_converts_a_mixed_tree_and_mirrors_structure() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source_root = temp.path().join("trip");
    let out = temp.path().join("out");

    // A generated PNG plus two renamed copies: the HEIC and the EXIF-rotated
    // JPEG keep their original bytes but not their original names.
    let top = write_png(&source_root.join("top.png"), 40, 30);
    let copied = copy_fixture("photo.heic", &source_root.join("sub").join("copied.heic"));
    let orient = copy_fixture(
        "orientation-6.jpg",
        &source_root.join("sub").join("orient.jpg"),
    );

    // The queue entry the UI sends: a canonical dropped folder plus each file's
    // path under it. `scan` is deliberately not involved; these are hand-built.
    let canonical_root = source_root.canonicalize().expect("canonical source folder");
    let items = vec![
        convert_item(&top, Some(&canonical_root), Some("top.png")),
        convert_item(&copied, Some(&canonical_root), Some("sub/copied.heic")),
        convert_item(&orient, Some(&canonical_root), Some("sub/orient.jpg")),
    ];

    let convert_settings = settings(OutputFormat::Jpeg, Quality::High, Some(&out), false);
    let outcome = convert_all(
        &items,
        &convert_settings,
        Arc::new(AtomicBool::new(false)),
        |_| {},
    );

    assert!(
        !outcome.cancelled,
        "no cancellation was requested, so the batch must not report one"
    );
    assert_eq!(
        outcome.results.len(),
        items.len(),
        "every queued file must produce exactly one result, got {:?}",
        outcome
            .results
            .iter()
            .map(|result| &result.path)
            .collect::<Vec<_>>()
    );

    // Workers finish in whatever order the image sizes and the pool decide, so
    // results are matched to inputs by source path, never by position.
    let by_path: HashMap<&str, &FileOutcome> = outcome
        .results
        .iter()
        .map(|result| (result.path.as_str(), result))
        .collect();
    assert_eq!(
        by_path.len(),
        outcome.results.len(),
        "two results claim the same source path: {:?}",
        outcome
            .results
            .iter()
            .map(|result| &result.path)
            .collect::<Vec<_>>()
    );

    // (source, relative path as the UI sends it, mirrored output, output size).
    // `orientation-6.jpg` is stored 40x30 but displays 30x40.
    let expected: [(&Path, &str, PathBuf, u32, u32); 3] = [
        (&top, "top.png", out.join("trip").join("top.jpg"), 40, 30),
        (
            &copied,
            "sub/copied.heic",
            out.join("trip").join("sub").join("copied.jpg"),
            40,
            30,
        ),
        (
            &orient,
            "sub/orient.jpg",
            out.join("trip").join("sub").join("orient.jpg"),
            30,
            40,
        ),
    ];

    for (source, relative, target, width, height) in &expected {
        let source_path = path_string(source);
        let result = by_path.get(source_path.as_str()).unwrap_or_else(|| {
            panic!(
                "{relative} ({source_path}) has no result; the batch returned {:?}",
                outcome
                    .results
                    .iter()
                    .map(|result| &result.path)
                    .collect::<Vec<_>>()
            )
        });

        assert_eq!(
            result.error, None,
            "{relative} must convert without an error"
        );
        assert!(!result.cancelled, "{relative} must not be cancelled");
        assert_eq!(
            result.output_path.as_deref(),
            Some(target.to_string_lossy().as_ref()),
            "{relative} must mirror the dropped folder into {}",
            target.display()
        );
        assert!(
            target.is_file(),
            "{} must exist on disk after the batch",
            target.display()
        );

        let kind = InputKind::from_path(target)
            .unwrap_or_else(|| panic!("{} is not a supported output format", target.display()));
        assert_eq!(kind, InputKind::Jpeg, "{} must be a JPEG", target.display());

        let decoded = image::open(target)
            .unwrap_or_else(|err| panic!("{} does not re-decode: {err}", target.display()));
        assert!(
            matches!(decoded.color(), image::ColorType::Rgb8),
            "{} must re-decode as Rgb8, got {:?}",
            target.display(),
            decoded.color()
        );
        assert_eq!(
            (decoded.width(), decoded.height()),
            (*width, *height),
            "{} must re-decode at {width}x{height}",
            target.display()
        );
        assert_eq!(
            (result.width, result.height),
            (Some(*width), Some(*height)),
            "{relative} must record its output dimensions as {width}x{height}"
        );
        assert_eq!(
            result.output_bytes,
            Some(file_len(target)),
            "{relative} must record the written file's length"
        );
        assert_eq!(
            result.input_bytes,
            file_len(source),
            "{relative} must record the source file's length"
        );
    }
}

/// Every regular file under `dir`, walked recursively and sorted. Used where a
/// test has to prove the batch wrote exactly what it reported and nothing else.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let entries = match std::fs::read_dir(&next) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries {
            let path = entry
                .unwrap_or_else(|err| {
                    panic!("could not read an entry under {}: {err}", next.display())
                })
                .path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

#[test]
fn batch_never_overwrites_the_source() {
    let temp = tempfile::tempdir().expect("tempdir");
    // Same format, same folder, overwrite on: the write target would be the
    // source itself if the planner did not protect it.
    let source = copy_fixture("photo.png", &temp.path().join("photo.png"));
    let original = std::fs::read(&source)
        .unwrap_or_else(|err| panic!("could not read {}: {err}", source.display()));

    let items = vec![convert_item(&source, None, None)];
    let convert_settings = settings(OutputFormat::Png, Quality::High, None, true);
    let outcome = convert_all(
        &items,
        &convert_settings,
        Arc::new(AtomicBool::new(false)),
        |_| {},
    );

    assert!(
        !outcome.cancelled,
        "no cancellation was requested, so the batch must not report one"
    );
    assert_eq!(
        outcome.results.len(),
        1,
        "one queued file must produce one result, got {:?}",
        outcome.results
    );
    let result = &outcome.results[0];
    assert_eq!(
        result.error, None,
        "photo.png must convert without an error, got {:?}",
        result.error
    );

    let expected = temp.path().join("photo (1).png");
    assert_eq!(
        result.output_path.as_deref(),
        Some(expected.to_string_lossy().as_ref()),
        "overwrite on must still steer the same-name target to the next free name"
    );
    assert!(
        expected.is_file(),
        "{} must exist after the batch",
        expected.display()
    );

    // The original's bytes are the whole point: identical format in the same
    // folder must never rewrite the source.
    let after = std::fs::read(&source)
        .unwrap_or_else(|err| panic!("could not re-read {}: {err}", source.display()));
    assert_eq!(
        after,
        original,
        "{} was rewritten by the batch; the source must stay byte-identical",
        source.display()
    );
    assert!(
        source.is_file(),
        "the original must still exist next to the new output"
    );
    let decoded = image::open(&source)
        .unwrap_or_else(|err| panic!("{} no longer decodes: {err}", source.display()));
    assert_eq!(
        (decoded.width(), decoded.height()),
        (40, 30),
        "the untouched source must still decode as the original 40x30 image"
    );
}

#[test]
fn batch_avoids_collisions_between_duplicate_names() {
    let temp = tempfile::tempdir().expect("tempdir");
    let first_root = temp.path().join("src").join("one").join("trip");
    let second_root = temp.path().join("src").join("two").join("trip");
    let out = temp.path().join("out");

    // Two different folders with the same basename, so both mirror to
    // `<out>/trip/` and both want `shot.png`.
    let first = write_png(&first_root.join("shot.png"), 40, 30);
    let second = write_png(&second_root.join("shot.png"), 40, 30);

    // A file already sitting on the unnumbered target, with distinct bytes so
    // a rewrite is detectable.
    let existing = write_png(&out.join("trip").join("shot.png"), 12, 9);
    let existing_bytes = std::fs::read(&existing)
        .unwrap_or_else(|err| panic!("could not read {}: {err}", existing.display()));

    let canonical_first = first_root
        .canonicalize()
        .expect("canonical first source folder");
    let canonical_second = second_root
        .canonicalize()
        .expect("canonical second source folder");
    let items = vec![
        convert_item(&first, Some(&canonical_first), Some("shot.png")),
        convert_item(&second, Some(&canonical_second), Some("shot.png")),
    ];

    let convert_settings = settings(OutputFormat::Png, Quality::High, Some(&out), false);
    let outcome = convert_all(
        &items,
        &convert_settings,
        Arc::new(AtomicBool::new(false)),
        |_| {},
    );

    assert!(
        !outcome.cancelled,
        "no cancellation was requested, so the batch must not report one"
    );
    assert_eq!(
        outcome.results.len(),
        2,
        "two queued files must produce two results, got {:?}",
        outcome.results
    );

    let mut targets: Vec<String> = Vec::new();
    for result in &outcome.results {
        assert_eq!(
            result.error, None,
            "{} must convert without an error, got {:?}",
            result.path, result.error
        );
        assert!(!result.cancelled, "{} must not be cancelled", result.path);
        let target = result
            .output_path
            .clone()
            .unwrap_or_else(|| panic!("{} came back without an output path", result.path));
        assert!(
            Path::new(&target).is_file(),
            "{} must exist on disk after the batch",
            target
        );
        targets.push(target);
    }
    targets.sort();

    // Which source gets (1) and which gets (2) depends on item order; the set
    // of targets does not.
    let expected = vec![
        path_string(&out.join("trip").join("shot (1).png")),
        path_string(&out.join("trip").join("shot (2).png")),
    ];
    assert_eq!(
        targets, expected,
        "duplicate names and the pre-existing file must push both outputs onto numbered names"
    );

    let after = std::fs::read(&existing)
        .unwrap_or_else(|err| panic!("could not re-read {}: {err}", existing.display()));
    assert_eq!(
        after,
        existing_bytes,
        "{} was rewritten; overwrite off must leave the pre-existing file alone",
        existing.display()
    );
    assert_eq!(
        files_under(&out).len(),
        3,
        "the batch must add two files beside the pre-existing one, got {:?}",
        files_under(&out)
    );
}

#[test]
fn batch_isolates_per_file_failures() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source_root = temp.path().join("queue");
    let out = temp.path().join("out");

    let ok = write_png(&source_root.join("ok.png"), 40, 30);
    // Corrupt fixtures copied under realistic input names; the committed
    // originals are only read.
    let truncated = copy_fixture("truncated.jpg", &source_root.join("broken.jpg"));
    let garbage = copy_fixture("garbage.png", &source_root.join("broken.png"));

    let canonical_root = source_root.canonicalize().expect("canonical source folder");
    let items = vec![
        convert_item(&ok, Some(&canonical_root), Some("ok.png")),
        convert_item(&truncated, Some(&canonical_root), Some("broken.jpg")),
        convert_item(&garbage, Some(&canonical_root), Some("broken.png")),
    ];

    let convert_settings = settings(OutputFormat::Png, Quality::High, Some(&out), false);
    let outcome = convert_all(
        &items,
        &convert_settings,
        Arc::new(AtomicBool::new(false)),
        |_| {},
    );

    assert!(
        !outcome.cancelled,
        "two decode failures are not a cancellation, so the batch must not report one"
    );
    assert_eq!(
        outcome.results.len(),
        3,
        "every queued file must produce one result even when its neighbours fail, got {:?}",
        outcome
            .results
            .iter()
            .map(|result| &result.path)
            .collect::<Vec<_>>()
    );

    let successes: Vec<&FileOutcome> = outcome
        .results
        .iter()
        .filter(|result| result.error.is_none())
        .collect();
    let failures: Vec<&FileOutcome> = outcome
        .results
        .iter()
        .filter(|result| result.error.is_some())
        .collect();

    assert_eq!(
        successes.len(),
        1,
        "only ok.png can decode, but the batch reported {} successes: {:?}",
        successes.len(),
        outcome
            .results
            .iter()
            .map(|result| (&result.path, &result.error))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        failures.len(),
        2,
        "both corrupt files must fail on their own, but the batch reported {} failures: {:?}",
        failures.len(),
        outcome
            .results
            .iter()
            .map(|result| (&result.path, &result.error))
            .collect::<Vec<_>>()
    );

    let success = successes[0];
    assert_eq!(
        success.path,
        path_string(&ok),
        "the one success must be ok.png, not a corrupt file"
    );
    let written = success
        .output_path
        .clone()
        .unwrap_or_else(|| panic!("ok.png succeeded without an output path"));
    assert_eq!(
        success.output_bytes,
        Some(file_len(Path::new(&written))),
        "the success must record the file it wrote"
    );

    for failure in &failures {
        let error = failure
            .error
            .as_deref()
            .unwrap_or_else(|| panic!("{} is in the failure list without an error", failure.path));
        assert!(
            !error.trim().is_empty(),
            "{} failed with an empty message; the row needs something to show",
            failure.path
        );
        assert_eq!(
            failure.output_path, None,
            "{} failed, so it must not claim an output path",
            failure.path
        );
    }

    // One output for one success: a failure that still wrote a partial file
    // would show up here.
    assert_eq!(
        files_under(&out),
        vec![PathBuf::from(&written)],
        "exactly one file may exist under {}, and it must be the success's output",
        out.display()
    );
}

#[test]
fn batch_can_be_cancelled_before_it_starts() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source_root = temp.path().join("queue");
    let out = temp.path().join("out");

    let sources: Vec<PathBuf> = ["one.png", "two.png", "three.png"]
        .iter()
        .map(|name| write_png(&source_root.join(name), 40, 30))
        .collect();

    let canonical_root = source_root.canonicalize().expect("canonical source folder");
    let items: Vec<ConvertItem> = sources
        .iter()
        .map(|source| {
            let relative = source
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| panic!("{} has no file name", source.display()));
            convert_item(source, Some(&canonical_root), Some(&relative))
        })
        .collect();

    // The flag is already set, so no worker ever reaches the decode-and-write
    // path. Mid-batch cancellation timing is explicitly not covered here.
    let cancel = Arc::new(AtomicBool::new(true));
    let convert_settings = settings(OutputFormat::Jpeg, Quality::High, Some(&out), false);
    let outcome = convert_all(&items, &convert_settings, cancel, |_| {});

    assert!(
        outcome.cancelled,
        "a pre-set cancel flag must make the batch report cancellation"
    );
    assert_eq!(
        outcome.results.len(),
        items.len(),
        "every queued file must still produce a result, got {:?}",
        outcome
            .results
            .iter()
            .map(|result| &result.path)
            .collect::<Vec<_>>()
    );

    let mut result_paths: Vec<&str> = outcome
        .results
        .iter()
        .map(|result| result.path.as_str())
        .collect();
    result_paths.sort_unstable();
    let mut input_paths: Vec<&str> = items.iter().map(|item| item.path.as_str()).collect();
    input_paths.sort_unstable();
    assert_eq!(
        result_paths, input_paths,
        "the cancelled results must be the queued files, one each"
    );

    for result in &outcome.results {
        assert!(
            result.cancelled,
            "{} must be marked cancelled before any work starts",
            result.path
        );
        assert_eq!(
            result.error, None,
            "{} was cancelled before it ran, so it must not report an error",
            result.path
        );
        assert_eq!(
            result.output_path, None,
            "{} was cancelled, so it must not claim an output path",
            result.path
        );
        assert_eq!(
            result.output_bytes, None,
            "{} was cancelled, so it must not claim written bytes",
            result.path
        );
    }

    assert!(
        files_under(&out).is_empty(),
        "a batch cancelled before it starts must write nothing, but {} contains {:?}",
        out.display(),
        files_under(&out)
    );
}

#[test]
fn batch_emits_one_progress_event_per_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source_root = temp.path().join("queue");
    let out = temp.path().join("out");

    let sources: Vec<PathBuf> = ["one.png", "two.png", "three.png"]
        .iter()
        .map(|name| write_png(&source_root.join(name), 40, 30))
        .collect();

    let canonical_root = source_root.canonicalize().expect("canonical source folder");
    let items: Vec<ConvertItem> = sources
        .iter()
        .map(|source| {
            let relative = source
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| panic!("{} has no file name", source.display()));
            convert_item(source, Some(&canonical_root), Some(&relative))
        })
        .collect();

    // The callback is `Fn`, so the events are collected behind a lock rather
    // than moved into it.
    let events: Mutex<Vec<ProgressEvent>> = Mutex::new(Vec::new());
    let convert_settings = settings(OutputFormat::Jpeg, Quality::High, Some(&out), false);
    let outcome = convert_all(
        &items,
        &convert_settings,
        Arc::new(AtomicBool::new(false)),
        |event| {
            events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(event);
        },
    );

    assert!(
        !outcome.cancelled,
        "no cancellation was requested, so the batch must not report one"
    );
    assert_eq!(
        outcome.results.len(),
        items.len(),
        "the event count is only meaningful if every file was converted, got {:?}",
        outcome
            .results
            .iter()
            .map(|result| &result.path)
            .collect::<Vec<_>>()
    );

    let events = events
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert_eq!(
        events.len(),
        items.len(),
        "three queued files must emit exactly three progress events"
    );
    for event in &events {
        assert_eq!(
            event.total,
            items.len(),
            "every event must report the batch total, but {} says {}",
            event.path,
            event.total
        );
    }

    // Workers finish out of order, so the counters and paths are compared as
    // sets: one event each, numbered 1..=3 with no repeats or gaps.
    let completed: BTreeSet<usize> = events.iter().map(|event| event.completed).collect();
    assert_eq!(
        completed,
        BTreeSet::from([1, 2, 3]),
        "completed counters must be exactly {{1,2,3}}, got {:?}",
        events
            .iter()
            .map(|event| event.completed)
            .collect::<Vec<_>>()
    );

    let mut event_paths: Vec<&str> = events.iter().map(|event| event.path.as_str()).collect();
    event_paths.sort_unstable();
    let mut input_paths: Vec<&str> = items.iter().map(|item| item.path.as_str()).collect();
    input_paths.sort_unstable();
    assert_eq!(
        event_paths, input_paths,
        "there must be one event per input path, no repeats and no strays"
    );

    for event in &events {
        assert_eq!(
            event.outcome.path, event.path,
            "an event about {} must carry that file's outcome, not {}",
            event.path, event.outcome.path
        );
    }
}

#[test]
fn progress_event_serializes_with_the_ipc_field_names() {
    // No Tauri handle is involved: the event is serialized directly, so this
    // pins the `convert-progress` payload the frontend receives.
    let outcome = FileOutcome {
        path: "/photos/trip/shot.png".into(),
        output_path: Some("/out/trip/shot.jpg".into()),
        output_bytes: Some(2_048),
        input_bytes: 4_096,
        width: Some(640),
        height: Some(480),
        error: Some("example error text".into()),
        cancelled: true,
    };
    let event = ProgressEvent {
        completed: 2,
        total: 3,
        path: "/photos/trip/shot.png".into(),
        outcome,
    };

    let value = serde_json::to_value(&event).expect("a progress event must serialize");
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("a progress event must serialize as an object, got {value}"));

    // Top-level shape: four keys, camelCase-safe names, real values.
    let expected_top = ["completed", "total", "path", "outcome"];
    for key in expected_top {
        assert!(
            object.contains_key(key),
            "the progress payload is missing the {key:?} key: {value}"
        );
    }
    assert_eq!(
        object.len(),
        expected_top.len(),
        "the progress payload must expose exactly {expected_top:?}, got {:?}",
        object.keys().collect::<Vec<_>>()
    );
    for key in object.keys() {
        assert!(
            !key.contains('_'),
            "top-level key {key:?} is snake_case; the IPC contract is camelCase"
        );
    }
    assert_eq!(object.get("completed"), Some(&serde_json::json!(2)));
    assert_eq!(object.get("total"), Some(&serde_json::json!(3)));
    assert_eq!(
        object.get("path"),
        Some(&serde_json::json!("/photos/trip/shot.png"))
    );

    // The nested outcome carries the per-file result the row renders.
    let outcome_value = object
        .get("outcome")
        .unwrap_or_else(|| panic!("the progress payload has no outcome: {value}"));
    let outcome_object = outcome_value
        .as_object()
        .unwrap_or_else(|| panic!("outcome must serialize as an object, got {outcome_value}"));

    let expected_outcome = [
        "path",
        "outputPath",
        "outputBytes",
        "inputBytes",
        "width",
        "height",
        "error",
        "cancelled",
    ];
    for key in expected_outcome {
        assert!(
            outcome_object.contains_key(key),
            "the outcome payload is missing the {key:?} key: {outcome_value}"
        );
    }
    assert_eq!(
        outcome_object.len(),
        expected_outcome.len(),
        "the outcome payload must expose exactly {expected_outcome:?}, got {:?}",
        outcome_object.keys().collect::<Vec<_>>()
    );
    for key in outcome_object.keys() {
        assert!(
            !key.contains('_'),
            "outcome key {key:?} is snake_case; the IPC contract is camelCase"
        );
    }

    assert_eq!(
        outcome_object.get("path"),
        Some(&serde_json::json!("/photos/trip/shot.png"))
    );
    assert_eq!(
        outcome_object.get("outputPath"),
        Some(&serde_json::json!("/out/trip/shot.jpg"))
    );
    assert_eq!(
        outcome_object.get("outputBytes"),
        Some(&serde_json::json!(2_048))
    );
    assert_eq!(
        outcome_object.get("inputBytes"),
        Some(&serde_json::json!(4_096))
    );
    assert_eq!(outcome_object.get("width"), Some(&serde_json::json!(640)));
    assert_eq!(outcome_object.get("height"), Some(&serde_json::json!(480)));
    assert_eq!(
        outcome_object.get("error"),
        Some(&serde_json::json!("example error text"))
    );
    assert_eq!(
        outcome_object.get("cancelled"),
        Some(&serde_json::json!(true))
    );
}
