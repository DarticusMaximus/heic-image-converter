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

    for index in order {
        if samples.len() >= MAX_SAMPLES {
            break;
        }
        let path = Path::new(&items[index].path);
        let Ok((bytes, width, height)) = convert::encode_bytes(path, settings) else {
            continue;
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
        return Err("could not read any of the selected images".into());
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
}
