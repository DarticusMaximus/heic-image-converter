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

// Capture of the lines `emit_panic` writes during tests. Each test runs on its
// own thread, so this thread-local sink keeps one test's contained panic out of
// another's view. Release builds carry none of it.
#[cfg(test)]
thread_local! {
    static PANIC_LOG: std::cell::RefCell<Vec<String>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Drain the panic lines captured on the calling thread. Test-only.
#[cfg(test)]
fn take_panic_log() -> Vec<String> {
    PANIC_LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// The downcast cause of a panic payload, or a placeholder when it is not a
/// string. `panic!` with a format produces `String`; a literal produces
/// `&'static str`; `panic_any` can produce anything.
fn panic_cause(payload: &(dyn std::any::Any + Send)) -> &str {
    if let Some(message) = payload.downcast_ref::<&str>() {
        message
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.as_str()
    } else {
        "<non-string panic payload>"
    }
}

/// Write one line describing a contained panic. Tests capture it in the
/// thread-local sink; everything else writes it to stderr.
#[cfg(test)]
fn emit_line(line: String) {
    PANIC_LOG.with(|log| log.borrow_mut().push(line));
}

#[cfg(not(test))]
fn emit_line(line: String) {
    eprintln!("{line}");
}

/// Report a contained panic once: the stage, the downcast cause and the seam
/// call site. The payload is still discarded afterwards — the seam's error
/// value and its `Display` text are unchanged.
fn emit_panic(stage: &str, payload: &(dyn std::any::Any + Send), at: &std::panic::Location<'_>) {
    emit_line(format!(
        "heic: the {stage} panicked: {} (at {at})",
        panic_cause(payload)
    ));
}

/// Run `f`, turning an escaped panic into a seam error so one bad file cannot
/// take the batch down.
///
/// Before the payload is dropped, [`emit_panic`] reports its message and the
/// seam call site (via `#[track_caller]`). The default panic hook still prints
/// the harness message to stderr in tests; the emitted line is what a windowed
/// release build can capture when a launcher keeps stderr.
#[track_caller]
fn guard<T>(stage: &'static str, f: impl FnOnce() -> T) -> Result<T, HeicError> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(value) => Ok(value),
        Err(payload) => {
            emit_panic(stage, payload.as_ref(), std::panic::Location::caller());
            Err(HeicError::Panic { stage })
        }
    }
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
    match image {
        // The common case: the caller already handed us RGB8, so write its
        // buffer straight through the encoder. `to_rgb8()` here would clone
        // the whole frame for nothing.
        DynamicImage::ImageRgb8(rgb) => {
            let encoder = heif::HeifEncoder::new_with_config(&mut buf, config);
            rgb.write_with_encoder(encoder)
                .map_err(|err| classify_encode_error(&err))?;
        }
        // Defensive: the seam still accepts any color type, the codec takes RGB8.
        other => {
            let encoder = heif::HeifEncoder::new_with_config(&mut buf, config);
            DynamicImage::ImageRgb8(other.to_rgb8())
                .write_with_encoder(encoder)
                .map_err(|err| classify_encode_error(&err))?;
        }
    }
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
    fn a_contained_panic_reports_its_cause() {
        take_panic_log();

        let result = guard("decoder", || -> () { panic!("sentinel-cause") });

        let err = result.expect_err("the panic must be contained");
        assert!(matches!(&err, HeicError::Panic { stage: "decoder" }));
        assert_eq!(
            err.to_string(),
            "the HEIC decoder crashed while processing this file",
            "the seam's Display text is frozen"
        );

        let lines = take_panic_log();
        assert_eq!(
            lines.len(),
            1,
            "exactly one line per contained panic: {lines:?}"
        );
        let line = &lines[0];
        assert!(line.contains("sentinel-cause"), "{line}");
        assert!(line.contains("decoder"), "{line}");
        assert!(
            line.contains("heic.rs:"),
            "the line must carry the seam call site: {line}"
        );

        // Show the captured line once when run with --nocapture.
        println!("captured panic line: {line}");
    }

    #[test]
    fn a_non_string_panic_payload_is_still_observable() {
        take_panic_log();

        let result = guard("encoder", || std::panic::panic_any(42u8));

        let err = result.expect_err("the non-string panic must be contained");
        assert!(matches!(&err, HeicError::Panic { stage: "encoder" }));
        assert_eq!(
            err.to_string(),
            "the HEIC encoder crashed while processing this file"
        );

        let lines = take_panic_log();
        assert_eq!(
            lines.len(),
            1,
            "exactly one line per contained panic: {lines:?}"
        );
        let line = &lines[0];
        assert!(line.contains("<non-string panic payload>"), "{line}");
        assert!(line.contains("encoder"), "{line}");
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

    /// `rgb` re-expressed as an opaque RGBA image: a non-RGB8 variant whose
    /// `to_rgb8()` must reproduce the same pixels, so `encode` takes the
    /// conversion fallback with identical input.
    fn opaque_rgba_twin(rgb: &image::RgbImage) -> DynamicImage {
        let rgba = image::RgbaImage::from_fn(rgb.width(), rgb.height(), |x, y| {
            let [r, g, b] = rgb.get_pixel(x, y).0;
            image::Rgba([r, g, b, 255])
        });
        DynamicImage::ImageRgba8(rgba)
    }

    /// The borrowed-buffer arm and the `to_rgb8()` fallback arm must produce
    /// the same bytes. The first encode goes through
    /// `DynamicImage::ImageRgb8`; the second wraps the same pixels in RGBA so
    /// the encoder only sees them after `to_rgb8()`.
    #[test]
    fn rgb8_source_and_fallback_paths_emit_identical_bytes() {
        let photo = decode(&fixture("photo.heic")).expect("decode photo.heic");
        for (label, image) in [("photo.heic", photo), ("sample_image", sample_image())] {
            let DynamicImage::ImageRgb8(rgb) = &image else {
                panic!("{label}: expected an RGB8 image, got {:?}", image.color());
            };
            let twin = opaque_rgba_twin(rgb);
            assert_eq!(
                twin.to_rgb8().as_raw(),
                rgb.as_raw(),
                "{label}: the fallback twin must be pixel-identical"
            );

            for quality in [30, 90] {
                for preset in [Preset::Fast, Preset::Slow] {
                    let borrowed = encode(&image, quality, preset)
                        .unwrap_or_else(|err| panic!("{label} q{quality} {preset:?}: {err}"));
                    let fallback = encode(&twin, quality, preset).unwrap_or_else(|err| {
                        panic!("{label} q{quality} {preset:?} fallback: {err}")
                    });
                    assert_eq!(
                        borrowed, fallback,
                        "{label} q{quality} {preset:?}: borrowed RGB8 buffer must encode \
                         identically to the to_rgb8() fallback"
                    );
                }
            }
        }
    }

    /// Encode straight from an `RgbImage`, which is what the `ImageRgb8` arm
    /// of `encode_inner` does, and confirm the bytes round-trip.
    #[test]
    fn rgb_buffer_encodes_directly() {
        let rgb = image::RgbImage::from_fn(64, 48, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x * 3 + y) % 256) as u8])
        });

        // The same settings `encode_inner` builds, so the two calls differ
        // only in how the borrowed buffer reaches the encoder.
        let config = heif::EncoderConfig {
            quality: 50,
            preset: codec_preset(Preset::Fast),
            chroma: heif::Chroma::Yuv420,
            bit_depth: heif::BitDepth::Eight,
            ..Default::default()
        };
        let mut direct = Vec::new();
        let encoder = heif::HeifEncoder::new_with_config(&mut direct, config);
        rgb.write_with_encoder(encoder)
            .expect("encode straight from an RgbImage");

        let seam = encode(&DynamicImage::ImageRgb8(rgb), 50, Preset::Fast)
            .expect("encode the Rgb8 variant through the seam");
        assert_eq!(
            direct, seam,
            "the Rgb8 variant must write the borrowed buffer through the seam"
        );

        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("direct.heic");
        std::fs::write(&path, &seam).expect("write encoded heic");
        let decoded = decode(&path).expect("decode direct encode");
        assert_eq!((decoded.width(), decoded.height()), (64, 48));
    }
}
