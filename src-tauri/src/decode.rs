//! Decoding for every supported input, including EXIF orientation.

use crate::formats::InputKind;
use image::ImageDecoder;
use image::{DynamicImage, Rgb, RgbImage};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Decode `path` into a `DynamicImage`, applying EXIF orientation for the
/// formats that carry it. HEIC rotation lives in the container (`irot`/`imir`)
/// and libheif already applies it, so applying EXIF again would double-rotate.
pub fn load(path: &Path, kind: InputKind) -> Result<DynamicImage, String> {
    let mut img = if kind.is_heic() {
        let file = File::open(path).map_err(|e| format!("could not open file: {e}"))?;
        let reader = BufReader::new(file);
        let decoder =
            heif::HeifDecoder::new(reader).map_err(|e| format!("HEIC decode failed: {e}"))?;
        DynamicImage::from_decoder(decoder).map_err(|e| format!("HEIC decode failed: {e}"))?
    } else {
        let reader = image::ImageReader::open(path)
            .map_err(|e| format!("could not open file: {e}"))?
            .with_guessed_format()
            .map_err(|e| format!("could not read image: {e}"))?;
        reader
            .decode()
            .map_err(|e| format!("could not decode image: {e}"))?
    };

    if !kind.is_heic() {
        img = apply_exif_orientation(img, path);
    }
    Ok(img)
}

/// Read only the dimensions, without decoding pixels where the format allows it.
pub fn dimensions(path: &Path, kind: InputKind) -> Option<(u32, u32)> {
    if kind.is_heic() {
        // libheif's Rust wrapper drains the reader before parsing the header,
        // so this costs a file read but skips the pixel decode.
        let data = std::fs::read(path).ok()?;
        let decoder = heif::HeifDecoder::new(std::io::Cursor::new(data)).ok()?;
        Some(decoder.dimensions())
    } else {
        let (width, height) = image::ImageReader::open(path)
            .ok()?
            .with_guessed_format()
            .ok()?
            .into_dimensions()
            .ok()?;
        // Report post-rotation dimensions so the list matches what a
        // conversion will actually produce.
        if matches!(read_orientation(path), 5..=8) {
            Some((height, width))
        } else {
            Some((width, height))
        }
    }
}

fn apply_exif_orientation(img: DynamicImage, path: &Path) -> DynamicImage {
    let orientation = read_orientation(path);
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

fn read_orientation(path: &Path) -> u32 {
    let Ok(file) = File::open(path) else {
        return 1;
    };
    let mut reader = BufReader::new(file);
    let Ok(exif) = exif::Reader::new().read_from_container(&mut reader) else {
        return 1;
    };
    exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|field| field.value.get_uint(0))
        .unwrap_or(1)
}

/// JPEG and HEIC carry no alpha channel, so transparent pixels are composited/// onto white instead of silently turning black.
pub fn flatten_to_rgb(img: &DynamicImage) -> RgbImage {
    if !img.color().has_alpha() {
        return img.to_rgb8();
    }
    let rgba = img.to_rgba8();
    let mut rgb = RgbImage::new(rgba.width(), rgba.height());
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = pixel[3] as u32;
        let blend =
            |channel: u8| -> u8 { ((channel as u32 * alpha + 255 * (255 - alpha)) / 255) as u8 };
        rgb.put_pixel(
            x,
            y,
            Rgb([blend(pixel[0]), blend(pixel[1]), blend(pixel[2])]),
        );
    }
    rgb
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// iPhone photos store rotation in the HEIC container (`irot`), not EXIF.
    /// libheif applies it during decode and reports post-rotation dimensions.
    #[test]
    fn applies_heic_container_rotation() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotated.heic");
        let kind = InputKind::from_path(&path).expect("heic fixture");
        assert_eq!(dimensions(&path, kind), Some((120, 160)));

        let image = load(&path, kind).expect("decode rotated heic");
        assert_eq!((image.width(), image.height()), (120, 160));
    }
}
