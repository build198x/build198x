//! Normalisation: dimension sanity cap, alpha compositing over the matte.

use build198x::convert::ConvertError;
use build198x::convert::normalise::{MAX_DIMENSION, normalise};
use image::{DynamicImage, RgbImage, RgbaImage};

#[test]
fn oversized_dimensions_are_rejected_with_a_typed_error() {
    let img = DynamicImage::ImageRgb8(RgbImage::new(MAX_DIMENSION + 1, 1));
    assert_eq!(
        normalise(&img, [0, 0, 0]),
        Err(ConvertError::DimensionsTooLarge {
            width: MAX_DIMENSION + 1,
            height: 1,
            max: MAX_DIMENSION,
        })
    );

    let img = DynamicImage::ImageRgb8(RgbImage::new(1, MAX_DIMENSION + 1));
    assert!(matches!(
        normalise(&img, [0, 0, 0]),
        Err(ConvertError::DimensionsTooLarge { .. })
    ));
}

#[test]
fn empty_image_is_rejected() {
    let img = DynamicImage::ImageRgb8(RgbImage::new(0, 0));
    assert_eq!(normalise(&img, [0, 0, 0]), Err(ConvertError::EmptyImage));
}

#[test]
fn alpha_composites_over_the_matte() {
    let mut rgba = RgbaImage::new(3, 1);
    rgba.put_pixel(0, 0, image::Rgba([200, 100, 50, 255])); // opaque
    rgba.put_pixel(1, 0, image::Rgba([200, 100, 50, 0])); // transparent
    rgba.put_pixel(2, 0, image::Rgba([255, 255, 255, 128])); // half
    let img = DynamicImage::ImageRgba8(rgba);

    let black = normalise(&img, [0, 0, 0]).expect("normalise");
    assert_eq!(black.pixels[0], [200, 100, 50]);
    assert_eq!(black.pixels[1], [0, 0, 0]);
    assert_eq!(black.pixels[2], [128, 128, 128]);

    let magenta = normalise(&img, [255, 0, 255]).expect("normalise");
    assert_eq!(magenta.pixels[0], [200, 100, 50]);
    assert_eq!(magenta.pixels[1], [255, 0, 255]);
}

#[test]
fn grey_input_expands_to_rgb() {
    let grey = image::GrayImage::from_pixel(2, 2, image::Luma([77]));
    let img = DynamicImage::ImageLuma8(grey);
    let out = normalise(&img, [0, 0, 0]).expect("normalise");
    assert!(out.pixels.iter().all(|&p| p == [77, 77, 77]));
}
