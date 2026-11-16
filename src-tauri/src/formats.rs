//! Supported formats, quality presets, and the mapping from friendly settings
//! to codec parameters.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// The input formats we accept. HEIC/HEIF gets its own path (libheif), every
/// other format goes through the `image` crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    Heic,
    Jpeg,
    Png,
    Webp,
    Tiff,
    Bmp,
    Gif,
}

impl InputKind {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "heic" | "heif" | "hif" => Self::Heic,
            "jpg" | "jpeg" | "jpe" | "jfif" => Self::Jpeg,
            "png" => Self::Png,
            "webp" => Self::Webp,
            "tif" | "tiff" => Self::Tiff,
            "bmp" => Self::Bmp,
            "gif" => Self::Gif,
            _ => return None,
        })
    }

    pub fn is_heic(self) -> bool {
        matches!(self, Self::Heic)
    }

    /// Lowercase label handed to the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Heic => "heic",
            Self::Jpeg => "jpeg",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Tiff => "tiff",
            Self::Bmp => "bmp",
            Self::Gif => "gif",
        }
    }
}

/// What we write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Png,
    Jpeg,
    Heic,
}

impl OutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Heic => "heic",
        }
    }
}

/// Four plain-language steps instead of a raw codec number. People pick
/// "what does this cost me", not "what CRF is my CRF".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    Smallest,
    Balanced,
    High,
    Maximum,
}

impl Quality {
    /// libjpeg quality 0-100.
    pub fn jpeg(self) -> u8 {
        match self {
            Self::Smallest => 60,
            Self::Balanced => 78,
            Self::High => 88,
            Self::Maximum => 95,
        }
    }

    /// libheif lossy quality 0-100 (x265 maps this onto CRF internally).
    pub fn heic(self) -> u8 {
        match self {
            Self::Smallest => 30,
            Self::Balanced => 50,
            Self::High => 68,
            Self::Maximum => 82,
        }
    }

    /// x265 speed preset. Quality presets also trade encode time for bytes:
    /// chasing the last few percent is not worth a 10x slower batch.
    pub fn heic_preset(self) -> heif::Preset {
        match self {
            Self::Smallest | Self::Balanced => heif::Preset::Fast,
            Self::High => heif::Preset::Medium,
            Self::Maximum => heif::Preset::Slow,
        }
    }
}

/// The user's conversion settings, as sent from the frontend.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertSettings {
    pub format: OutputFormat,
    pub quality: Quality,
    /// Longest-edge cap in pixels. `None` keeps the original dimensions.
    pub max_dimension: Option<u32>,
    /// `None` writes next to each source file.
    pub output_dir: Option<String>,
    pub overwrite: bool,
}

impl ConvertSettings {
    /// Clamp anything the UI should not be able to send.
    pub fn sanitized(mut self) -> Self {
        self.max_dimension = match self.max_dimension {
            Some(px) if px >= 16 => Some(px),
            _ => None,
        };
        self.output_dir = self
            .output_dir
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detects_input_kinds() {
        let cases = [
            ("a.HEIC", Some(InputKind::Heic)),
            ("a.heif", Some(InputKind::Heic)),
            ("a.jpeg", Some(InputKind::Jpeg)),
            ("a.PNG", Some(InputKind::Png)),
            ("a.webp", Some(InputKind::Webp)),
            ("a.tiff", Some(InputKind::Tiff)),
            ("a.txt", None),
            ("noext", None),
        ];
        for (name, expected) in cases {
            assert_eq!(
                InputKind::from_path(&PathBuf::from(name)),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn quality_steps_increase() {
        let steps = [
            Quality::Smallest,
            Quality::Balanced,
            Quality::High,
            Quality::Maximum,
        ];
        for pair in steps.windows(2) {
            assert!(pair[1].jpeg() > pair[0].jpeg());
            assert!(pair[1].heic() > pair[0].heic());
        }
    }
}
