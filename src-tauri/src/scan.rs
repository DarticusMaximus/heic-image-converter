//! Folder expansion, metadata probing, and thumbnail generation for the
//! file list.

use crate::decode::{self, flatten_to_rgb};
use crate::formats::InputKind;
use base64::Engine;
use image::ImageEncoder;
use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const MAX_FILES: usize = 20_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFile {
    /// Canonical path; doubles as the stable id used by the UI.
    pub id: String,
    pub path: String,
    pub name: String,
    pub kind: String,
    pub bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// For files found inside a dropped folder: that folder.
    pub root: Option<String>,
    /// Path relative to `root`, used to mirror the tree on output.
    pub relative: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFile {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub files: Vec<SourceFile>,
    pub skipped: Vec<SkippedFile>,
    pub truncated: bool,
}

#[derive(Debug)]
struct Candidate {
    path: PathBuf,
    root: Option<PathBuf>,
}

pub fn scan(paths: &[String]) -> ScanResult {
    scan_with_limit(paths, MAX_FILES)
}

/// Same as [`scan`], with an injectable file cap so truncation can be tested
/// without creating `MAX_FILES` files.
pub(crate) fn scan_with_limit(paths: &[String], max_files: usize) -> ScanResult {
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut skipped: Vec<SkippedFile> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut truncated = false;

    'outer: for raw in paths {
        let path = PathBuf::from(raw);
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(err) => {
                skipped.push(SkippedFile {
                    path: raw.clone(),
                    reason: format!("cannot open: {err}"),
                });
                continue;
            }
        };

        if canonical.is_dir() {
            for entry in WalkDir::new(&canonical)
                .follow_links(false)
                .sort_by_file_name()
                .into_iter()
                .filter_entry(|entry| entry.depth() == 0 || !is_hidden(entry.path()))
            {
                let Ok(entry) = entry else { continue };
                if !entry.file_type().is_file() {
                    continue;
                }
                let file_path = entry.path();
                if InputKind::from_path(file_path).is_none() {
                    continue;
                }
                if candidates.len() >= max_files {
                    truncated = true;
                    break 'outer;
                }
                if !seen.insert(file_path.to_path_buf()) {
                    continue;
                }
                candidates.push(Candidate {
                    path: file_path.to_path_buf(),
                    root: Some(canonical.clone()),
                });
            }
        } else if canonical.is_file() {
            if InputKind::from_path(&canonical).is_none() {
                skipped.push(SkippedFile {
                    path: raw.clone(),
                    reason: "not a supported image format".into(),
                });
                continue;
            }
            if candidates.len() >= max_files {
                truncated = true;
                break;
            }
            if seen.insert(canonical.clone()) {
                candidates.push(Candidate {
                    path: canonical,
                    root: None,
                });
            }
        } else {
            skipped.push(SkippedFile {
                path: raw.clone(),
                reason: "not a file or folder".into(),
            });
        }
    }

    let mut files: Vec<SourceFile> = candidates
        .par_iter()
        .map(|candidate| describe(candidate))
        .collect();

    // Deterministic order: group by folder root, then path.
    files.sort_by(|a, b| {
        let root_a = a.root.as_deref().unwrap_or("");
        let root_b = b.root.as_deref().unwrap_or("");
        root_a.cmp(root_b).then_with(|| a.path.cmp(&b.path))
    });

    ScanResult {
        files,
        skipped,
        truncated,
    }
}

fn describe(candidate: &Candidate) -> SourceFile {
    let path = &candidate.path;
    let kind = InputKind::from_path(path).expect("filtered during scan");
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let dims = decode::dimensions(path, kind);
    let relative = candidate.root.as_ref().and_then(|root| {
        path.strip_prefix(root)
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    });

    SourceFile {
        id: path.to_string_lossy().into_owned(),
        path: path.to_string_lossy().into_owned(),
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".into()),
        kind: kind.label().to_string(),
        bytes,
        width: dims.map(|d| d.0),
        height: dims.map(|d| d.1),
        root: candidate
            .root
            .as_ref()
            .map(|r| r.to_string_lossy().into_owned()),
        relative,
    }
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.') || n == "Thumbs.db" || n == "desktop.ini")
        .unwrap_or(false)
}

/// Small JPEG data URL for the file list. Decoding is the expensive part, so
/// the UI calls this lazily with a bounded number of requests in flight.
pub fn thumbnail(path: &str, max: u32) -> Result<String, String> {
    let path = Path::new(path);
    let kind = InputKind::from_path(path).ok_or_else(|| "unsupported format".to_string())?;
    let img = decode::load(path, kind)?;
    let edge = max.clamp(32, 512);
    let thumb = if img.width() <= edge && img.height() <= edge {
        img
    } else {
        img.resize(edge, edge, image::imageops::FilterType::Triangle)
    };
    let rgb = flatten_to_rgb(&thumb);

    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 74)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("thumbnail encode failed: {e}"))?;

    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&buf)
    ))
}
