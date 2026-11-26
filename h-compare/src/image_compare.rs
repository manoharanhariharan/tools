//! Image decoding and pixel-difference heatmap generation.

use image::{DynamicImage, GenericImageView};

#[derive(Debug, Clone)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ImageCompareData {
    pub left: Option<ImageData>,
    pub right: Option<ImageData>,
    pub diff: Option<ImageData>,
    pub changed_pixels: usize,
    pub dimensions_match: bool,
}

pub fn compare_images(left_bytes: &[u8], right_bytes: &[u8]) -> Result<ImageCompareData, String> {
    let left = decode_image(left_bytes, "left")?;
    let right = decode_image(right_bytes, "right")?;
    if left.is_none() && right.is_none() {
        return Err("neither side could be decoded as an image".to_string());
    }
    let dimensions_match = match (&left, &right) {
        (Some(left), Some(right)) => left.width == right.width && left.height == right.height,
        _ => false,
    };
    let (diff, changed_pixels) = match (&left, &right) {
        (Some(left), Some(right)) if dimensions_match => {
            let mut diff = vec![0u8; left.rgba.len()];
            let mut changed = 0usize;
            for (index, (a, b)) in left
                .rgba
                .chunks_exact(4)
                .zip(right.rgba.chunks_exact(4))
                .enumerate()
            {
                let max_delta = a
                    .iter()
                    .zip(b.iter())
                    .map(|(x, y)| x.abs_diff(*y))
                    .max()
                    .unwrap_or(0);
                if max_delta == 0 {
                    continue;
                }
                changed += 1;
                let intensity = max_delta as f32 / 255.0;
                let start = index * 4;
                diff[start] = 255;
                diff[start + 1] = (200.0 * (1.0 - intensity)) as u8;
                diff[start + 2] = 0;
                diff[start + 3] = (70.0 + 185.0 * intensity).round() as u8;
            }
            (
                Some(ImageData {
                    width: left.width,
                    height: left.height,
                    rgba: diff,
                }),
                changed,
            )
        }
        _ => (None, 0),
    };
    Ok(ImageCompareData {
        left,
        right,
        diff,
        changed_pixels,
        dimensions_match,
    })
}

fn decode_image(bytes: &[u8], side: &str) -> Result<Option<ImageData>, String> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let image = image::load_from_memory(bytes)
        .map_err(|error| format!("could not decode {side} image: {error}"))?;
    Ok(Some(image_to_data(image)))
}

fn image_to_data(image: DynamicImage) -> ImageData {
    let (width, height) = image.dimensions();
    let rgba = image.to_rgba8().into_raw();
    ImageData {
        width,
        height,
        rgba,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn png(width: u32, height: u32, pixels: &[[u8; 4]]) -> Vec<u8> {
        let mut image = RgbaImage::new(width, height);
        for (index, pixel) in pixels.iter().enumerate() {
            let x = index as u32 % width;
            let y = index as u32 / width;
            image.put_pixel(x, y, Rgba(*pixel));
        }
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .expect("encode png");
        bytes.into_inner()
    }

    #[test]
    fn reports_changed_pixels_and_builds_a_heatmap() {
        let left = png(
            2,
            2,
            &[
                [255, 0, 0, 255],
                [0, 255, 0, 255],
                [0, 0, 255, 255],
                [255, 255, 255, 255],
            ],
        );
        let right = png(
            2,
            2,
            &[
                [255, 0, 0, 255],
                [0, 255, 0, 255],
                [0, 0, 255, 255],
                [0, 0, 0, 255],
            ],
        );
        let compare = compare_images(&left, &right).expect("compare images");
        assert!(compare.dimensions_match);
        assert_eq!(compare.changed_pixels, 1);
        let diff = compare.diff.expect("heatmap");
        assert_eq!((diff.width, diff.height), (2, 2));
        assert!(diff.rgba[3 * 4 + 3] > 0);
        assert_eq!(diff.rgba[3], 0);
    }

    #[test]
    fn skips_heatmap_when_dimensions_differ() {
        let left = png(1, 1, &[[255, 255, 255, 255]]);
        let right = png(2, 1, &[[255, 255, 255, 255], [0, 0, 0, 255]]);
        let compare = compare_images(&left, &right).expect("compare images");
        assert!(!compare.dimensions_match);
        assert!(compare.diff.is_none());
        assert_eq!(compare.changed_pixels, 0);
    }

    #[test]
    fn handles_one_missing_side() {
        let left = png(1, 1, &[[255, 255, 255, 255]]);
        let compare = compare_images(&left, &[]).expect("compare images");
        assert!(compare.left.is_some());
        assert!(compare.right.is_none());
        assert!(compare.diff.is_none());
    }
}
