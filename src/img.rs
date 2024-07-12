extern crate alumina;
extern crate bytevec;
extern crate image;
extern crate rand;
extern crate webp;

use crate::network::sr_net;
use alumina::graph::*;
use alumina::shape::*;
use anyhow::{Context, Result};
use bytevec::ByteDecodable;
use image::imageops::{resize, FilterType};
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageFormat, Rgba, RgbaImage};
use std::path::Path;
use webp::Encoder;

const IMAGENET_PARAMS: &'static [u8] = include_bytes!("imagenet.rsr");

pub fn process_image(
    input: &Path,
    output: &Path,
    compress: bool,
    upscale: bool,
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
        let encoder = Encoder::from_image(&img)
            .map_err(|e| anyhow::anyhow!("Failed to create WebP encoder: {}", e))?;
        let webp = encoder.encode(85f32);
        let webp_output_path = output.with_extension("webp");
        std::fs::write(&webp_output_path, &*webp).context("Failed to write WebP image")?;
        return Ok(ImageFormat::WebP);
    }

    // For cases 3 and 4, we need to upscale
    let factor = 3; // Hardcode factor 3
    let img2 = img.clone();
    let b_w_img = black_and_white(img);
    let b_w_img_upscaled = resize(
        &b_w_img,
        b_w_img.width() * factor,
        b_w_img.height() * factor,
        FilterType::Triangle,
    );
    let transparent_img = white_to_transparent(img2);
    let ai_img = ai_upscale(transparent_img, factor as usize);
    let upscaled_img = combine_black(ai_img, DynamicImage::ImageRgba8(b_w_img_upscaled));

    // Case 3: Upscale, no compression (PNG)
    if !compress {
        upscaled_img
            .save_with_format(output, ImageFormat::Png)
            .context("Failed to save PNG image")?;
        return Ok(ImageFormat::Png);
    }

    // Case 4: Upscale and compress (WebP)
    let encoder = Encoder::from_image(&upscaled_img)
        .map_err(|e| anyhow::anyhow!("Failed to create WebP encoder: {}", e))?;
    let webp = encoder.encode(85f32);
    let webp_output_path = output.with_extension("webp");
    std::fs::write(&webp_output_path, &*webp).context("Failed to write WebP image")?;
    Ok(ImageFormat::WebP)
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

fn black_and_white(img: DynamicImage) -> DynamicImage {
    let white = Rgba([255, 255, 255, 255]);
    let black = Rgba([0, 0, 0, 255]);
    let mut output_img: RgbaImage = ImageBuffer::new(img.width(), img.height());
    for (x, y, pixel) in img.pixels() {
        if pixel != white {
            output_img.put_pixel(x, y, black);
        } else {
            output_img.put_pixel(x, y, white);
        }
    }
    DynamicImage::ImageRgba8(output_img)
}

fn white_to_transparent(img: DynamicImage) -> DynamicImage {
    let white = Rgba([255, 255, 255, 255]);
    let transparent = Rgba([0, 0, 0, 0]);
    let mut img2: RgbaImage = ImageBuffer::new(img.width(), img.height());
    for (x, y, pixel) in img.pixels() {
        if pixel == white {
            img2.put_pixel(x, y, transparent);
        } else {
            img2.put_pixel(x, y, pixel);
        }
    }
    DynamicImage::ImageRgba8(img2)
}

///Combines AI upscaled image with wrong background. Merges it with black and with
///gaussian upscaled image where white parts of that image acts as alpha layer
fn combine_black(img2: DynamicImage, black_white_img: DynamicImage) -> DynamicImage {
    let mut img_buf: RgbaImage =
        ImageBuffer::new(black_white_img.width(), black_white_img.height());
    for (x, y, pixel) in img2.pixels() {
        let mut color = pixel;
        color[3] = 255 - black_white_img.get_pixel(x, y)[0];
        img_buf.put_pixel(x, y, color);
    }
    DynamicImage::ImageRgba8(img_buf)
}
