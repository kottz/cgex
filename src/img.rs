extern crate alumina;
extern crate bytevec;
extern crate image;
extern crate rand;

use crate::network::sr_net;
use alumina::graph::*;
use alumina::shape::*;
use anyhow::{Context, Result};
use bytevec::ByteDecodable;
use image::imageops::{resize, FilterType};
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageFormat, Rgba, RgbaImage};
use std::path::Path;

const IMAGENET_PARAMS: &'static [u8] = include_bytes!("imagenet.rsr");

pub fn process_image(
    input: &Path,
    output: &Path,
    compress: bool,
    upscale: bool,
    transparent_color: [u8; 3],
) -> Result<ImageFormat> {
    let img = image::open(input).context("Failed to open input image")?;

    // Case 1: No upscale, no compression (original BMP)
    if !upscale && !compress {
        img.save_with_format(output, ImageFormat::Bmp)
            .context("Failed to save BMP image")?;
        return Ok(ImageFormat::Bmp);
    }

    // Case 2: No upscale, with compression (small WebP)
    if !upscale && compress {
        img.save_with_format(output, ImageFormat::WebP)
            .context("Failed to save BMP image")?;
        return Ok(ImageFormat::WebP);
    }

    // For cases 3 and 4, we need to upscale
    let factor = 3; // Hardcode factor 3
    let img2 = img.clone();
    let b_w_img = background_and_foreground(img, transparent_color);
    let b_w_img_upscaled = resize(
        &b_w_img,
        b_w_img.width() * factor,
        b_w_img.height() * factor,
        FilterType::Triangle,
    );
    let transparent_img = background_to_transparent(img2, transparent_color);
    let ai_img = ai_upscale(transparent_img, factor as usize);
    let upscaled_img = combine_background(ai_img, DynamicImage::ImageRgba8(b_w_img_upscaled));

    let format: ImageFormat = if compress {
        ImageFormat::WebP
    } else {
        ImageFormat::Png
    };

    upscaled_img
        .save_with_format(output, format)
        .context("Failed to save image")?;
    Ok(format)
}

fn background_and_foreground(img: DynamicImage, transparent_color: [u8; 3]) -> DynamicImage {
    let background = Rgba([
        transparent_color[0],
        transparent_color[1],
        transparent_color[2],
        255,
    ]);
    let foreground = Rgba([0, 0, 0, 255]);
    let mut output_img: RgbaImage = ImageBuffer::new(img.width(), img.height());
    for (x, y, pixel) in img.pixels() {
        if pixel.0[0] == transparent_color[0]
            && pixel.0[1] == transparent_color[1]
            && pixel.0[2] == transparent_color[2]
        {
            output_img.put_pixel(x, y, background);
        } else {
            output_img.put_pixel(x, y, foreground);
        }
    }
    DynamicImage::ImageRgba8(output_img)
}

fn background_to_transparent(img: DynamicImage, transparent_color: [u8; 3]) -> DynamicImage {
    let transparent = Rgba([0, 0, 0, 0]);
    let mut img2: RgbaImage = ImageBuffer::new(img.width(), img.height());
    for (x, y, pixel) in img.pixels() {
        if pixel.0[0] == transparent_color[0]
            && pixel.0[1] == transparent_color[1]
            && pixel.0[2] == transparent_color[2]
        {
            img2.put_pixel(x, y, transparent);
        } else {
            img2.put_pixel(x, y, pixel);
        }
    }
    DynamicImage::ImageRgba8(img2)
}

fn combine_background(img2: DynamicImage, background_img: DynamicImage) -> DynamicImage {
    let mut img_buf: RgbaImage = ImageBuffer::new(background_img.width(), background_img.height());
    for (x, y, pixel) in img2.pixels() {
        let mut color = pixel;
        color[3] = 255 - background_img.get_pixel(x, y)[0];
        img_buf.put_pixel(x, y, color);
    }
    DynamicImage::ImageRgba8(img_buf)
}

fn ai_upscale(input_image: DynamicImage, factor: usize) -> DynamicImage {
    let (params, mut graph) = (
        <Vec<f32>>::decode::<u32>(IMAGENET_PARAMS).expect("ByteVec conversion failed"),
        sr_net(factor, None),
    );

    let rgba_image = input_image.to_rgba8();
    let (width, height) = rgba_image.dimensions();

    // Convert RGBA to RGB
    let rgb_pixels: Vec<f32> = rgba_image
        .pixels()
        .flat_map(|p| {
            [
                p[0] as f32 / 255.0,
                p[1] as f32 / 255.0,
                p[2] as f32 / 255.0,
            ]
        })
        .collect();

    let mut input = NodeData::new_blank(DataShape::new(
        3, // Assuming CHANNELS is 3 for RGB
        &[width as usize, height as usize],
        1,
    ));

    // Copy the RGB pixel data into input.values
    input.values.copy_from_slice(&rgb_pixels);

    let output = graph.forward(1, vec![input], &params).remove(0);

    // Convert the output back to RGBA
    let output_pixels: Vec<u8> = output
        .values
        .chunks(3)
        .flat_map(|chunk| {
            let r = (chunk[0] * 255.0) as u8;
            let g = (chunk[1] * 255.0) as u8;
            let b = (chunk[2] * 255.0) as u8;
            [r, g, b, 255] // Add alpha channel
        })
        .collect();

    let output_image =
        RgbaImage::from_raw(width * factor as u32, height * factor as u32, output_pixels)
            .expect("Failed to create output image");

    DynamicImage::ImageRgba8(output_image)
}
