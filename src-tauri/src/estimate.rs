//! Size estimation. Rather than guess at bits-per-pixel from a table, we
//! actually encode a few of the user's own images at the chosen settings and
//! extrapolate by pixel count. It costs a second or two and is directionally
//! honest, which a static heuristic is not.

use crate::convert;
use crate::formats::ConvertSettings;
use serde::{Deserialize, Serialize};
use std::path::Path;

const MAX_SAMPLES: usize = 3;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimateItem {
    pub path: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleEstimate {
    pub name: String,
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimateResult {
    /// Encoded bytes per output pixel, measured from the samples.
    pub bytes_per_pixel: f64,
    pub estimated_total_bytes: u64,
    pub total_pixels: u64,
    pub sampled: usize,
    pub samples: Vec<SampleEstimate>,
}

/// Pick samples spread across the list so a batch of mixed content does not
/// get estimated from three nearly identical photos.
fn pick_samples(items: &[EstimateItem]) -> Vec<usize> {
    let eligible: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.width.is_some() && item.height.is_some())
        .map(|(index, _)| index)
        .collect();
    if eligible.len() <= MAX_SAMPLES {
        return eligible;
    }
    (0..MAX_SAMPLES)
        .map(|i| eligible[i * (eligible.len() - 1) / (MAX_SAMPLES - 1)])
        .collect()
}

pub fn estimate(
    items: &[EstimateItem],
    settings: &ConvertSettings,
) -> Result<EstimateResult, String> {
    if items.is_empty() {
        return Err("nothing to estimate".into());
    }

    let mut order = pick_samples(items);
    if order.is_empty() {
        return Err("could not read image dimensions".into());
    }
    // If a sampled file turns out to be corrupt, fall back to the remaining
    // images instead of failing the whole estimate.
    for (index, item) in items.iter().enumerate() {
        if item.width.is_some() && item.height.is_some() && !order.contains(&index) {
            order.push(index);
        }
    }

    let mut samples = Vec::new();
    let mut sampled_pixels: u64 = 0;
    let mut sampled_bytes: u64 = 0;
    // The first concrete failure is carried to the terminal error. Skipping the
    // bad sample is deliberate, but if nothing at all can be encoded the cause
    // must be reported as what it is — an encoder that cannot start must not
    // read as "unreadable images". One cause is enough: it describes the
    // condition that made every sample fail.
    let mut first_error: Option<String> = None;

    for index in order {
        if samples.len() >= MAX_SAMPLES {
            break;
        }
        let path = Path::new(&items[index].path);
        let (bytes, width, height) = match convert::encode_bytes(path, settings) {
            Ok(sample) => sample,
            Err(err) => {
                if first_error.is_none() {
                    first_error = Some(err);
                }
                continue;
            }
        };
        sampled_pixels += width as u64 * height as u64;
        sampled_bytes += bytes.len() as u64;
        samples.push(SampleEstimate {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "image".into()),
            input_bytes: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
            output_bytes: bytes.len() as u64,
            width,
            height,
        });
    }

    if samples.is_empty() {
        return Err(match first_error {
            Some(cause) => format!("could not estimate from any selected image: {cause}"),
            None => "could not read any of the selected images".into(),
        });
    }

    let bytes_per_pixel = sampled_bytes as f64 / sampled_pixels.max(1) as f64;

    // Total output pixels across the whole batch, after any resize cap.
    let total_pixels: u64 = items
        .iter()
        .filter_map(|item| item.width.zip(item.height))
        .map(|(w, h)| target_pixels(w, h, settings.max_dimension))
        .sum();

    let estimated_total_bytes = (bytes_per_pixel * total_pixels as f64).round() as u64;

    Ok(EstimateResult {
        bytes_per_pixel,
        estimated_total_bytes,
        total_pixels,
        sampled: samples.len(),
        samples,
    })
}

pub fn target_pixels(width: u32, height: u32, max_dimension: Option<u32>) -> u64 {
    let Some(max) = max_dimension else {
        return width as u64 * height as u64;
    };
    let longest = width.max(height);
    if longest <= max {
        return width as u64 * height as u64;
    }
    let scale = max as f64 / longest as f64;
    let w = (width as f64 * scale).round().max(1.0) as u64;
    let h = (height as f64 * scale).round().max(1.0) as u64;
    w * h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::{OutputFormat, Quality};

    fn settings(format: OutputFormat) -> ConvertSettings {
        ConvertSettings {
            format,
            quality: Quality::Balanced,
            max_dimension: None,
            output_dir: None,
            overwrite: false,
        }
    }

    fn fixture(name: &str) -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
            .to_string_lossy()
            .into_owned()
    }

    fn item(path: impl Into<String>) -> EstimateItem {
        EstimateItem {
            path: path.into(),
            width: Some(40),
            height: Some(30),
        }
    }

    #[test]
    fn target_pixels_respects_the_cap() {
        assert_eq!(target_pixels(4000, 3000, Some(2000)), 2000 * 1500);
        assert_eq!(target_pixels(4000, 3000, None), 4000 * 3000);
        assert_eq!(target_pixels(1000, 800, Some(2000)), 1000 * 800);
    }

    #[test]
    fn samples_are_spread_across_a_long_list() {
        let items: Vec<EstimateItem> = (0..10)
            .map(|i| EstimateItem {
                path: format!("/x/{i}.jpg"),
                width: Some(100),
                height: Some(100),
            })
            .collect();
        assert_eq!(pick_samples(&items), vec![0, 4, 9]);
    }

    #[test]
    fn an_all_failed_estimate_reports_the_underlying_cause() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut items = Vec::new();
        for name in ["one.heic", "two.heic", "three.heic"] {
            let path = dir.path().join(name);
            std::fs::write(&path, b"0123456789").expect("write non-image bytes");
            items.push(item(path.to_string_lossy().into_owned()));
        }

        let err =
            estimate(&items, &settings(OutputFormat::Jpeg)).expect_err("no sample can encode");

        assert!(
            err.starts_with("could not estimate from any selected image: "),
            "{err}"
        );
        // The seam's own name for the failure must survive, not a read-flavoured
        // substitute for it.
        assert!(err.contains("HEIC decode failed"), "{err}");
        assert!(err.contains("damaged or not a HEIC image"), "{err}");
        assert_ne!(err, "could not read any of the selected images");
    }

    #[test]
    fn a_partially_failing_set_still_estimates_from_the_working_samples() {
        let items = [
            item(fixture("garbage.png")),
            item(fixture("photo.png")),
            item(fixture("alpha.png")),
        ];

        let result = estimate(&items, &settings(OutputFormat::Jpeg))
            .expect("working samples should carry the estimate");

        assert_eq!(result.sampled, 2);
        let names: Vec<&str> = result.samples.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["photo.png", "alpha.png"]);
        assert!(result.estimated_total_bytes > 0);
    }

    #[test]
    fn dimension_only_items_keep_the_dimension_error() {
        let items = vec![EstimateItem {
            path: "/x/a.jpg".into(),
            width: None,
            height: None,
        }];

        let err = estimate(&items, &settings(OutputFormat::Jpeg)).expect_err("no eligible samples");

        assert_eq!(err, "could not read image dimensions");
    }

    #[test]
    fn an_empty_selection_keeps_the_nothing_to_estimate_error() {
        let err = estimate(&[], &settings(OutputFormat::Jpeg)).expect_err("nothing selected");

        assert_eq!(err, "nothing to estimate");
    }
}
