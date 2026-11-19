//! The single adapter between this product and `heif-rs`. No other module
//! names the crate; every HEIC failure mode gets a name here.

use image::{DynamicImage, ImageDecoder, ImageError};
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// Our vocabulary for the codec's speed/size tradeoff, so callers never name
/// the crate's preset enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Fast,
    Medium,
    Slow,
}

/// What went wrong on the HEIC path.
#[derive(Debug)]
pub enum HeicError {
    /// The container cannot be read or has no primary image handle.
    CorruptContainer(String),
    /// The container parsed but pixel decoding failed.
    Decode(String),
    /// libheif could not initialise an HEVC encoder.
    EncoderUnavailable(String),
    /// Any other encode failure.
    Encode(String),
    /// A panic escaped the decoder or encoder.
    Panic { stage: &'static str },
    /// The file could not be read.
    Io(String),
}

impl fmt::Display for HeicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CorruptContainer(detail) => write!(
                f,
                "HEIC decode failed: this file is damaged or not a HEIC image ({detail})"
            ),
            Self::Decode(detail) => write!(f, "HEIC decode failed: {detail}"),
            Self::EncoderUnavailable(detail) => {
                write!(
                    f,
                    "HEIC encode failed: no HEVC encoder is available ({detail})"
                )
            }
            Self::Encode(detail) => write!(f, "HEIC encode failed: {detail}"),
            Self::Panic { stage } => {
                write!(f, "the HEIC {stage} crashed while processing this file")
            }
            Self::Io(detail) => write!(f, "could not open file: {detail}"),
        }
    }
}

/// Run `f`, turning an escaped panic into a seam error so one bad file cannot
/// take the batch down.
fn guard<T>(stage: &'static str, f: impl FnOnce() -> T) -> Result<T, HeicError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .map_err(|_| HeicError::Panic { stage })
}

fn classify_container_error(err: ImageError) -> HeicError {
    match err {
        ImageError::IoError(err) => HeicError::Io(err.to_string()),
        other => HeicError::CorruptContainer(other.to_string()),
    }
}

fn classify_decode_error(err: ImageError) -> HeicError {
    match err {
        ImageError::IoError(err) => HeicError::Io(err.to_string()),
        other => HeicError::Decode(other.to_string()),
    }
}

fn classify_encode_error(err: &ImageError) -> HeicError {
    let detail = err.to_string();
    if detail.contains("encoder initialization failed") {
        // heif-rs renders `HeifError::EncoderInit` with that prefix; the
        // coupling is deliberate and pinned by the encoder-init test.
        HeicError::EncoderUnavailable(detail)
    } else {
        HeicError::Encode(detail)
    }
}

/// Decode the primary image of the HEIC at `path`. libheif applies the
/// container rotation, so callers must not apply EXIF orientation on top.
pub fn decode(path: &Path) -> Result<DynamicImage, HeicError> {
    let file = File::open(path).map_err(|err| HeicError::Io(err.to_string()))?;
    decode_from(BufReader::new(file))
}

fn decode_from<R: Read>(reader: R) -> Result<DynamicImage, HeicError> {
    guard("decoder", || -> Result<DynamicImage, HeicError> {
        let decoder = heif::HeifDecoder::new(reader).map_err(classify_container_error)?;
        DynamicImage::from_decoder(decoder).map_err(classify_decode_error)
    })?
}

/// Read only the dimensions of the HEIC at `path`, skipping the pixel decode.
/// The wrapper drains the reader for the header, so this costs one file read
/// but no pixel decode.
pub fn dimensions(path: &Path) -> Result<(u32, u32), HeicError> {
    let file = File::open(path).map_err(|err| HeicError::Io(err.to_string()))?;
    guard("decoder", || -> Result<(u32, u32), HeicError> {
        let decoder =
            heif::HeifDecoder::new(BufReader::new(file)).map_err(classify_container_error)?;
        Ok(decoder.dimensions())
    })?
}

/// Encode `image` to HEIC bytes at `quality` with `preset`, 4:2:0 and 8-bit.
/// Callers composite alpha onto a background first: HEIC has no alpha channel.
pub fn encode(image: &DynamicImage, quality: u8, preset: Preset) -> Result<Vec<u8>, HeicError> {
    guard("encoder", || encode_inner(image, quality, preset))?
}

fn encode_inner(image: &DynamicImage, quality: u8, preset: Preset) -> Result<Vec<u8>, HeicError> {
    let config = heif::EncoderConfig {
        quality,
        preset: codec_preset(preset),
        chroma: heif::Chroma::Yuv420,
        bit_depth: heif::BitDepth::Eight,
        ..Default::default()
    };
    let mut buf: Vec<u8> = Vec::new();
    let encoder = heif::HeifEncoder::new_with_config(&mut buf, config);
    // Defensive: the seam accepts any color type, the codec takes RGB8.
    DynamicImage::ImageRgb8(image.to_rgb8())
        .write_with_encoder(encoder)
        .map_err(|err| classify_encode_error(&err))?;
    Ok(buf)
}

/// Our preset vocabulary mapped onto the crate's enum.
fn codec_preset(preset: Preset) -> heif::Preset {
    match preset {
        Preset::Fast => heif::Preset::Fast,
        Preset::Medium => heif::Preset::Medium,
        Preset::Slow => heif::Preset::Slow,
    }
}

/// Bits per channel of the source image. Test-only: the fixture corpus is the
/// only consumer, so a release build carries none of this.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitDepth {
    Eight,
    Ten,
    Twelve,
}

#[cfg(test)]
impl BitDepth {
    /// The per-channel depth as the fixture manifest records it.
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Eight => 8,
            Self::Ten => 10,
            Self::Twelve => 12,
        }
    }
}

/// What the container header says about the image, without decoding pixels.
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub struct ProbeInfo {
    pub width: u32,
    pub height: u32,
    pub bit_depth: BitDepth,
}

/// Read only the header facts of the HEIC at `path`, skipping the pixel decode.
/// Test-only: the fixture corpus is the only consumer.
#[cfg(test)]
pub fn probe(path: &Path) -> Result<ProbeInfo, HeicError> {
    let data = std::fs::read(path).map_err(|err| HeicError::Io(err.to_string()))?;
    guard("decoder", || -> Result<ProbeInfo, HeicError> {
        let info =
            heif::probe(&data).map_err(|err| HeicError::CorruptContainer(err.to_string()))?;
        Ok(ProbeInfo {
            width: info.width,
            height: info.height,
            // The wrapper collapses every depth it cannot express to `Eight`,
            // which is a wrapper limitation, not a seam failure.
            bit_depth: match info.bit_depth {
                heif::BitDepth::Ten => BitDepth::Ten,
                heif::BitDepth::Twelve => BitDepth::Twelve,
                _ => BitDepth::Eight,
            },
        })
    })?
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::error::{DecodingError, EncodingError, ImageFormatHint};
    use std::io;
    use std::path::PathBuf;

    fn decoding_error(err: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> ImageError {
        ImageError::Decoding(DecodingError::new(
            ImageFormatHint::Name("HEIF".into()),
            err,
        ))
    }

    fn encoding_error(err: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> ImageError {
        ImageError::Encoding(EncodingError::new(
            ImageFormatHint::Name("HEIF".into()),
            err,
        ))
    }

    #[test]
    fn classify_container_error_marks_bad_input() {
        let decoded = decoding_error(heif::HeifError::Decode("invalid box".into()));
        assert!(matches!(
            classify_container_error(decoded),
            HeicError::CorruptContainer(_)
        ));

        let io = ImageError::IoError(io::Error::new(io::ErrorKind::NotFound, "no such file"));
        assert!(matches!(classify_container_error(io), HeicError::Io(_)));
    }

    #[test]
    fn classify_decode_error_marks_pixel_failures() {
        let decoded = decoding_error(heif::HeifError::Decode("bad tile data".into()));
        assert!(matches!(
            classify_decode_error(decoded),
            HeicError::Decode(_)
        ));

        let io = ImageError::IoError(io::Error::new(io::ErrorKind::UnexpectedEof, "short read"));
        assert!(matches!(classify_decode_error(io), HeicError::Io(_)));
    }

    #[test]
    fn encoder_init_failure_is_encoder_unavailable() {
        let err = encoding_error(heif::HeifError::EncoderInit("no x265".into()));
        match classify_encode_error(&err) {
            HeicError::EncoderUnavailable(detail) => {
                assert!(detail.contains("encoder initialization failed"), "{detail}");
            }
            other => panic!("expected EncoderUnavailable, got {other:?}"),
        }
    }

    #[test]
    fn other_encode_failures_stay_encode() {
        let err = encoding_error(heif::HeifError::Encode("out of memory".into()));
        assert!(matches!(classify_encode_error(&err), HeicError::Encode(_)));
    }

    struct PanickingReader;

    impl Read for PanickingReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            panic!("reader exploded");
        }
    }

    #[test]
    fn a_panicking_reader_is_a_decoder_panic() {
        let result = decode_from(PanickingReader);
        assert!(matches!(result, Err(HeicError::Panic { stage: "decoder" })));
    }

    #[test]
    fn an_encoder_panic_is_an_encoder_panic() {
        let result = guard("encoder", || -> () { panic!("encoder exploded") });
        assert!(matches!(result, Err(HeicError::Panic { stage: "encoder" })));
    }

    #[test]
    fn error_messages_are_readable() {
        let variants = [
            HeicError::CorruptContainer("detail".into()),
            HeicError::Decode("detail".into()),
            HeicError::EncoderUnavailable("detail".into()),
            HeicError::Encode("detail".into()),
            HeicError::Panic { stage: "decoder" },
            HeicError::Io("detail".into()),
        ];
        for err in &variants {
            assert!(!err.to_string().is_empty(), "{err:?}");
        }

        let naming_heic = [
            HeicError::CorruptContainer("detail".into()),
            HeicError::Decode("detail".into()),
            HeicError::EncoderUnavailable("detail".into()),
            HeicError::Encode("detail".into()),
            HeicError::Panic { stage: "encoder" },
        ];
        for err in &naming_heic {
            assert!(err.to_string().contains("HEIC"), "{err:?}");
        }
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    /// The only file allowed to name the crate.
    const SEAM_FILE: &str = "heic.rs";

    /// Files whose text names the crate, returned in input order. A module has
    /// leaked the seam when its text contains `heif::` or the `use heif`
    /// import form; anything else is our own type.
    fn scan_for_heif_references(files: &[(String, String)]) -> Vec<String> {
        files
            .iter()
            .filter(|(_, text)| text.contains("heif::") || text.contains("use heif"))
            .map(|(path, _)| path.clone())
            .collect()
    }

    /// Every `.rs` file under `root`, recursively. Entries are sorted at each
    /// level so the walk order, and therefore any report built from it, is
    /// deterministic.
    fn rust_sources_under(root: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let mut entries: Vec<PathBuf> = std::fs::read_dir(root)
            .unwrap_or_else(|err| panic!("read {}: {err}", root.display()))
            .map(|entry| entry.expect("directory entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                found.extend(rust_sources_under(&path));
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                found.push(path);
            }
        }
        found
    }

    /// Offending modules under `src-tauri/src`, with the seam itself skipped.
    fn heif_references_in_src_tree() -> Vec<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let files: Vec<(String, String)> = rust_sources_under(&root)
            .into_iter()
            .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some(SEAM_FILE))
            .map(|path| {
                let text = std::fs::read_to_string(&path)
                    .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
                (path.to_string_lossy().into_owned(), text)
            })
            .collect();
        scan_for_heif_references(&files)
    }

    #[test]
    fn scan_flags_an_offender() {
        let files = vec![
            (
                "src-tauri/src/leaky.rs".to_string(),
                "let decoder = heif::HeifDecoder::new(reader);".to_string(),
            ),
            (
                "src-tauri/src/clean.rs".to_string(),
                "use image::{DynamicImage, ImageReader};".to_string(),
            ),
            (
                "src-tauri/src/imports_the_crate.rs".to_string(),
                "use heif;".to_string(),
            ),
        ];

        assert_eq!(
            scan_for_heif_references(&files),
            vec![
                "src-tauri/src/leaky.rs".to_string(),
                "src-tauri/src/imports_the_crate.rs".to_string(),
            ]
        );
    }

    #[test]
    fn only_the_seam_module_names_the_heif_crate() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        assert!(
            rust_sources_under(&root).len() > 1,
            "the walker found no source tree under {}",
            root.display()
        );

        let offenders = heif_references_in_src_tree();
        assert!(
            offenders.is_empty(),
            "only {SEAM_FILE} may name the heif crate; these modules do: {}",
            offenders.join(", ")
        );
    }

    #[test]
    fn plain_heic_decodes_to_eight_bit_rgb() {
        let image = decode(&fixture("photo.heic")).expect("decode photo.heic");
        assert_eq!((image.width(), image.height()), (40, 30));
        assert_eq!(image.color(), image::ColorType::Rgb8);
    }

    #[test]
    fn twelve_bit_heic_decodes_to_sixteen_bit_pixels() {
        let image = decode(&fixture("deep12.heic")).expect("decode deep12.heic");
        assert_eq!((image.width(), image.height()), (40, 30));
        assert_eq!(image.color(), image::ColorType::Rgb16);
    }

    #[test]
    fn dimensions_match_the_decoded_image() {
        for name in ["photo.heic", "deep12.heic", "burst.heic", "rotated.heic"] {
            let path = fixture(name);
            let image = decode(&path).unwrap_or_else(|err| panic!("{name}: {err}"));
            let dims = dimensions(&path).unwrap_or_else(|err| panic!("{name}: {err}"));
            assert_eq!(dims, (image.width(), image.height()), "{name}");
        }
    }

    #[test]
    fn a_truncated_container_is_a_readable_error() {
        let bytes = std::fs::read(fixture("photo.heic")).expect("read photo.heic");
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("truncated.heic");
        std::fs::write(&path, &bytes[..64]).expect("write truncated container");

        let err = decode(&path).expect_err("truncated container must fail");
        assert!(
            matches!(err, HeicError::CorruptContainer(_)),
            "expected CorruptContainer, got {err:?}"
        );
        assert!(err.to_string().starts_with("HEIC decode failed"), "{err}");
    }

    #[test]
    fn ascii_bytes_are_a_corrupt_container() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("not-an-image.heic");
        std::fs::write(&path, b"0123456789").expect("write ascii bytes");

        assert!(
            matches!(decode(&path), Err(HeicError::CorruptContainer(_))),
            "expected CorruptContainer for ascii bytes"
        );
    }

    #[test]
    fn probe_reports_the_source_depth() {
        let photo = probe(&fixture("photo.heic")).expect("probe photo.heic");
        assert_eq!((photo.width, photo.height), (40, 30));
        assert_eq!(photo.bit_depth, BitDepth::Eight);

        let deep = probe(&fixture("deep12.heic")).expect("probe deep12.heic");
        assert_eq!((deep.width, deep.height), (40, 30));
        assert_eq!(deep.bit_depth, BitDepth::Twelve);
    }

    #[test]
    fn bit_depth_maps_to_manifest_depths() {
        assert_eq!(BitDepth::Eight.as_u8(), 8);
        assert_eq!(BitDepth::Ten.as_u8(), 10);
        assert_eq!(BitDepth::Twelve.as_u8(), 12);
    }

    #[test]
    fn a_multi_image_file_decodes_its_primary_image() {
        let image = decode(&fixture("burst.heic")).expect("decode burst.heic");
        assert_eq!(
            (image.width(), image.height()),
            (40, 30),
            "primary image is 40x30; the 20x15 auxiliary must be ignored"
        );
    }

    fn sample_image() -> DynamicImage {
        DynamicImage::ImageRgb8(image::RgbImage::from_fn(320, 240, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
        }))
    }

    #[test]
    fn encoding_round_trips() {
        let image = sample_image();
        let bytes = encode(&image, 50, Preset::Fast).expect("encode sample");

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("round-trip.heic");
        std::fs::write(&path, &bytes).expect("write encoded heic");

        let decoded = decode(&path).expect("decode round-tripped heic");
        assert_eq!((decoded.width(), decoded.height()), (320, 240));
    }

    #[test]
    fn encode_settings_reach_the_codec() {
        let image = sample_image();

        let low = encode(&image, 30, Preset::Fast).expect("encode quality 30");
        let high = encode(&image, 90, Preset::Fast).expect("encode quality 90");
        assert_ne!(low, high, "quality must reach the codec");

        let fast = encode(&image, 50, Preset::Fast).expect("encode preset Fast");
        let slow = encode(&image, 50, Preset::Slow).expect("encode preset Slow");
        assert_ne!(fast, slow, "preset must reach the codec");
    }
}
