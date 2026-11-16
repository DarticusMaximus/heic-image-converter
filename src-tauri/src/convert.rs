//! The conversion pipeline: decode -> optional resize -> encode -> write.

use crate::decode::{self, flatten_to_rgb};
use crate::formats::{ConvertSettings, InputKind, OutputFormat};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::{DynamicImage, ExtendedColorType, ImageEncoder};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertItem {
    pub path: String,
    /// Folder that was dropped, if this file was discovered inside one.
    pub root: Option<String>,
    /// Path relative to `root`.
    pub relative: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOutcome {
    pub path: String,
    pub output_path: Option<String>,
    pub output_bytes: Option<u64>,
    pub input_bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub error: Option<String>,
    pub cancelled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionOutcome {
    pub results: Vec<FileOutcome>,
    pub cancelled: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    completed: usize,
    total: usize,
    path: String,
    outcome: FileOutcome,
}

/// Run the whole batch. One image at a time per worker; HEIC gets fewer
/// workers because x265 already saturates the CPU on its own.
pub fn convert_all(
    app: &AppHandle,
    items: &[ConvertItem],
    settings: &ConvertSettings,
    cancel: Arc<AtomicBool>,
) -> ConversionOutcome {
    let total = items.len();
    let completed = AtomicUsize::new(0);
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let workers = if settings.format == OutputFormat::Heic {
        (cores / 6).clamp(1, 3)
    } else {
        (cores / 2).clamp(1, 6)
    };

    let pool = rayon::ThreadPoolBuilder::new().num_threads(workers).build();

    // Resolve every output path before any worker starts, so two images with
    // the same name can't race for the same file.
    let targets = plan_targets(items, settings);

    let run = || {
        items
            .par_iter()
            .zip(targets.par_iter())
            .map(|(item, target)| {
                let outcome = if cancel.load(Ordering::Relaxed) {
                    FileOutcome {
                        path: item.path.clone(),
                        output_path: None,
                        output_bytes: None,
                        input_bytes: file_size(&item.path),
                        width: None,
                        height: None,
                        error: None,
                        cancelled: true,
                    }
                } else {
                    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                        convert_one(item, settings, target)
                    }));
                    match result {
                        Ok(outcome) => outcome,
                        Err(_) => FileOutcome {
                            path: item.path.clone(),
                            output_path: None,
                            output_bytes: None,
                            input_bytes: file_size(&item.path),
                            width: None,
                            height: None,
                            error: Some("internal error while converting this file".into()),
                            cancelled: false,
                        },
                    }
                };

                let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = app.emit(
                    "convert-progress",
                    ProgressEvent {
                        completed: done,
                        total,
                        path: item.path.clone(),
                        outcome: outcome.clone(),
                    },
                );
                outcome
            })
            .collect::<Vec<_>>()
    };

    let results = match pool {
        Ok(pool) => pool.install(run),
        Err(_) => run(),
    };

    ConversionOutcome {
        cancelled: results.iter().any(|r| r.cancelled),
        results,
    }
}

/// Decode, scale and encode a single file without writing anything. Used by
/// both the real conversion and the size estimator.
pub fn encode_bytes(
    path: &Path,
    settings: &ConvertSettings,
) -> Result<(Vec<u8>, u32, u32), String> {
    let kind = InputKind::from_path(path).ok_or_else(|| "unsupported format".to_string())?;
    let image = decode::load(path, kind)?;
    let image = resize(image, settings.max_dimension);
    let (width, height) = (image.width(), image.height());
    let bytes = encode(image, settings)?;
    Ok((bytes, width, height))
}

fn convert_one(item: &ConvertItem, settings: &ConvertSettings, target: &Path) -> FileOutcome {
    let mut outcome = FileOutcome {
        path: item.path.clone(),
        output_path: None,
        output_bytes: None,
        input_bytes: file_size(&item.path),
        width: None,
        height: None,
        error: None,
        cancelled: false,
    };

    let source = Path::new(&item.path);
    match encode_bytes(source, settings) {
        Ok((bytes, width, height)) => {
            if let Some(parent) = target.parent() {
                if let Err(err) = std::fs::create_dir_all(parent) {
                    outcome.error = Some(format!("could not create output folder: {err}"));
                    return outcome;
                }
            }
            match std::fs::write(target, &bytes) {
                Ok(()) => {
                    outcome.output_path = Some(target.to_string_lossy().into_owned());
                    outcome.output_bytes = Some(bytes.len() as u64);
                    outcome.width = Some(width);
                    outcome.height = Some(height);
                }
                Err(err) => outcome.error = Some(format!("could not write file: {err}")),
            }
        }
        Err(err) => outcome.error = Some(err),
    }

    outcome
}

fn resize(image: DynamicImage, max_dimension: Option<u32>) -> DynamicImage {
    let Some(max) = max_dimension else {
        return image;
    };
    let longest = image.width().max(image.height());
    if longest <= max {
        return image;
    }
    image.resize(max, max, image::imageops::FilterType::Lanczos3)
}

fn encode(image: DynamicImage, settings: &ConvertSettings) -> Result<Vec<u8>, String> {
    let mut buf: Vec<u8> = Vec::new();
    match settings.format {
        OutputFormat::Png => {
            // libheif happily hands back 10/12-bit pixels, and a 16-bit PNG is
            // twice the size for detail no screen will show. PNG output is
            // always 8-bit; grayscale stays grayscale to keep it small.
            let image = to_eight_bit(image);
            PngEncoder::new(&mut buf)
                .write_image(
                    image.as_bytes(),
                    image.width(),
                    image.height(),
                    image.color().into(),
                )
                .map_err(|e| format!("PNG encode failed: {e}"))?;
        }
        OutputFormat::Jpeg => {
            let rgb = flatten_to_rgb(&image);
            JpegEncoder::new_with_quality(&mut buf, settings.quality.jpeg())
                .write_image(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    ExtendedColorType::Rgb8,
                )
                .map_err(|e| format!("JPEG encode failed: {e}"))?;
        }
        OutputFormat::Heic => {
            let rgb = DynamicImage::ImageRgb8(flatten_to_rgb(&image));
            let config = heif::EncoderConfig {
                quality: settings.quality.heic(),
                preset: settings.quality.heic_preset(),
                chroma: heif::Chroma::Yuv420,
                ..Default::default()
            };
            let encoder = heif::HeifEncoder::new_with_config(&mut buf, config);
            rgb.write_with_encoder(encoder)
                .map_err(|e| format!("HEIC encode failed: {e}"))?;
        }
    }
    Ok(buf)
}

fn to_eight_bit(image: DynamicImage) -> DynamicImage {
    use image::ColorType::*;
    match image.color() {
        L16 => DynamicImage::ImageLuma8(image.to_luma8()),
        La16 => DynamicImage::ImageLumaA8(image.to_luma_alpha8()),
        Rgb16 => DynamicImage::ImageRgb8(image.to_rgb8()),
        Rgba16 => DynamicImage::ImageRgba8(image.to_rgba8()),
        Rgb32F => DynamicImage::ImageRgb8(image.to_rgb8()),
        Rgba32F => DynamicImage::ImageRgba8(image.to_rgba8()),
        _ => image,
    }
}

/// Where an output lands, before collision handling.
fn base_target(item: &ConvertItem, settings: &ConvertSettings) -> PathBuf {
    let source = Path::new(&item.path);
    let extension = settings.format.extension();
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".into());

    let directory = match &settings.output_dir {
        Some(out) => {
            let mut dir = PathBuf::from(out);
            if let Some(root) = &item.root {
                if let Some(name) = Path::new(root).file_name() {
                    dir.push(name);
                }
            }
            if let Some(relative) = &item.relative {
                if let Some(parent) = Path::new(relative).parent() {
                    if !parent.as_os_str().is_empty() {
                        dir.push(parent);
                    }
                }
            }
            dir
        }
        None => source
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
    };

    directory.join(format!("{stem}.{extension}"))
}

/// Resolve a collision-free path for every item, in order, reserving names as
/// we go so parallel workers can never write the same file.
fn plan_targets(items: &[ConvertItem], settings: &ConvertSettings) -> Vec<PathBuf> {
    let mut reserved: HashSet<PathBuf> = HashSet::new();
    items
        .iter()
        .map(|item| {
            let candidate = base_target(item, settings);
            let chosen = unique_path(
                &candidate,
                Path::new(&item.path),
                settings.overwrite,
                &reserved,
            );
            reserved.insert(chosen.clone());
            chosen
        })
        .collect()
}

fn unique_path(
    candidate: &Path,
    source: &Path,
    overwrite: bool,
    reserved: &HashSet<PathBuf>,
) -> PathBuf {
    let free = |path: &Path| !reserved.contains(path) && !same_path(path, source) && !path.exists();
    if free(candidate)
        || (overwrite && !same_path(candidate, source) && !reserved.contains(candidate))
    {
        return candidate.to_path_buf();
    }

    let parent = candidate.parent().unwrap_or_else(|| Path::new("."));
    let stem = candidate
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".into());
    let extension = candidate
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    for n in 1..10_000 {
        let name = if extension.is_empty() {
            format!("{stem} ({n})")
        } else {
            format!("{stem} ({n}).{extension}")
        };
        let path = parent.join(name);
        if free(&path) || (overwrite && !same_path(&path, source) && !reserved.contains(&path)) {
            return path;
        }
    }
    candidate.to_path_buf()
}

fn same_path(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn file_size(path: &str) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(
        format: OutputFormat,
        output_dir: Option<&str>,
        overwrite: bool,
    ) -> ConvertSettings {
        ConvertSettings {
            format,
            quality: crate::formats::Quality::Balanced,
            max_dimension: None,
            output_dir: output_dir.map(str::to_string),
            overwrite,
        }
    }

    #[test]
    fn mirrors_dropped_folder_structure() {
        let item = ConvertItem {
            path: "/photos/trip/day1/IMG_0001.HEIC".into(),
            root: Some("/photos/trip".into()),
            relative: Some("day1/IMG_0001.HEIC".into()),
        };
        let s = settings(OutputFormat::Jpeg, Some("/out"), false);
        let path = plan_targets(&[item], &s);
        assert_eq!(path[0], PathBuf::from("/out/trip/day1/IMG_0001.jpg"));
    }

    #[test]
    fn single_file_goes_straight_into_output_dir() {
        let item = ConvertItem {
            path: "/photos/IMG_0001.HEIC".into(),
            root: None,
            relative: None,
        };
        let s = settings(OutputFormat::Png, Some("/out"), false);
        let path = plan_targets(&[item], &s);
        assert_eq!(path[0], PathBuf::from("/out/IMG_0001.png"));
    }

    #[test]
    fn same_name_and_format_never_overwrites_the_source() {
        let item = ConvertItem {
            path: "/photos/IMG_0001.heic".into(),
            root: None,
            relative: None,
        };
        let s = settings(OutputFormat::Heic, None, true);
        let path = plan_targets(&[item], &s);
        assert_eq!(path[0], PathBuf::from("/photos/IMG_0001 (1).heic"));
    }

    #[test]
    fn duplicate_names_get_distinct_targets() {
        let dir = tempfile::tempdir().expect("tempdir");
        let items = vec![
            ConvertItem {
                path: "/photos/one/trip/IMG_1.HEIC".into(),
                root: Some("/photos/one/trip".into()),
                relative: Some("IMG_1.HEIC".into()),
            },
            ConvertItem {
                path: "/photos/two/trip/IMG_1.HEIC".into(),
                root: Some("/photos/two/trip".into()),
                relative: Some("IMG_1.HEIC".into()),
            },
        ];
        let s = settings(OutputFormat::Jpeg, dir.path().to_str(), false);
        let targets = plan_targets(&items, &s);
        assert_eq!(targets[0], dir.path().join("trip/IMG_1.jpg"));
        assert_eq!(targets[1], dir.path().join("trip/IMG_1 (1).jpg"));
    }

    #[test]
    fn clamps_absurd_max_dimension() {
        let mut s = settings(OutputFormat::Jpeg, None, false);
        s.max_dimension = Some(0);
        assert_eq!(s.sanitized().max_dimension, None);
        let mut s = settings(OutputFormat::Jpeg, None, false);
        s.max_dimension = Some(4096);
        assert_eq!(s.sanitized().max_dimension, Some(4096));
    }

    fn sample_png(dir: &Path, name: &str, width: u32, height: u32) -> PathBuf {
        let path = dir.join(name);
        let image = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
        });
        image.save(&path).expect("write sample png");
        path
    }

    #[test]
    fn converts_png_to_heic_and_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = sample_png(dir.path(), "sample.png", 320, 240);
        let out = dir.path().join("out");

        let item = ConvertItem {
            path: source.to_string_lossy().into_owned(),
            root: None,
            relative: None,
        };

        for format in [OutputFormat::Heic, OutputFormat::Jpeg, OutputFormat::Png] {
            let mut s = settings(format, Some(out.to_str().unwrap()), false);
            s.quality = crate::formats::Quality::High;
            let target = plan_targets(std::slice::from_ref(&item), &s);
            let outcome = convert_one(&item, &s, &target[0]);
            assert_eq!(outcome.error, None, "format {format:?}");
            let written = PathBuf::from(outcome.output_path.expect("output path"));
            assert!(written.exists());

            // The result must decode back at the right size and format.
            let kind = InputKind::from_path(&written).expect("known output format");
            let (width, height) = decode::dimensions(&written, kind).expect("read back dims");
            assert_eq!((width, height), (320, 240));
            assert_eq!(written.extension().unwrap(), format.extension());
        }
    }

    #[test]
    fn png_output_is_always_eight_bit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("deep.png");
        let deep = image::ImageBuffer::<image::Rgb<u16>, Vec<u16>>::from_fn(320, 240, |x, y| {
            image::Rgb([(x * 200) as u16, (y * 200) as u16, 4000])
        });
        image::DynamicImage::ImageRgb16(deep)
            .save(&source)
            .expect("write 16-bit png");

        let item = ConvertItem {
            path: source.to_string_lossy().into_owned(),
            root: None,
            relative: None,
        };
        let s = settings(OutputFormat::Png, dir.path().to_str(), false);
        let target = plan_targets(std::slice::from_ref(&item), &s);
        let outcome = convert_one(&item, &s, &target[0]);
        assert_eq!(outcome.error, None);
        let written = PathBuf::from(outcome.output_path.expect("output path"));
        let decoded = image::open(&written).expect("decode png");
        assert!(matches!(
            decoded.color(),
            image::ColorType::Rgb8
                | image::ColorType::Rgba8
                | image::ColorType::L8
                | image::ColorType::La8
        ));
    }

    #[test]
    fn resize_caps_the_longest_edge() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = sample_png(dir.path(), "big.png", 800, 400);
        let item = ConvertItem {
            path: source.to_string_lossy().into_owned(),
            root: None,
            relative: None,
        };
        let mut s = settings(
            OutputFormat::Jpeg,
            Some(dir.path().to_str().unwrap()),
            false,
        );
        s.max_dimension = Some(200);

        let target = plan_targets(std::slice::from_ref(&item), &s);
        let outcome = convert_one(&item, &s, &target[0]);
        assert_eq!(outcome.error, None);
        assert_eq!((outcome.width, outcome.height), (Some(200), Some(100)));
    }
}
