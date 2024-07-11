extern crate alumina;
extern crate bytevec;
extern crate image;
extern crate rand;

use std::path::Path;

use crate::network::sr_net;
use alumina::graph::*;
use alumina::shape::*;
use alumina::supplier::imagefolder::*;
use anyhow::Result;
use bytevec::ByteDecodable;
use image::imageops::{resize, FilterType};
use image::{DynamicImage, GenericImage, ImageBuffer, RgbaImage};

const IMAGENET_PARAMS: &'static [u8] = include_bytes!("imagenet.rsr");

///Performs AI super sampling, adds alpha layer based on white parts of image.
pub fn process_image(input: &Path, output: &Path) -> Result<()> {
    let factor = 3; //Hardcode factor 3

    let img = image::open(&Path::new(input)).unwrap();
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
    let combine_ai = combine_black(ai_img, DynamicImage::ImageRgba8(b_w_img_upscaled));

    Ok(combine_ai.to_rgba().save(output)?)
}

fn ai_upscale(input_image: DynamicImage, factor: usize) -> DynamicImage {
    let (params, mut graph) = (
        <Vec<f32>>::decode::<u32>(IMAGENET_PARAMS).expect("ByteVec conversion failed"),
        sr_net(factor, None),
    );
    let mut input = NodeData::new_blank(DataShape::new(
        CHANNELS,
        &[
            input_image.dimensions().0 as usize,
            input_image.dimensions().1 as usize,
        ],
        1,
    ));
    img_to_data(&mut input.values, &input_image);
    let output = graph.forward(1, vec![input], &params).remove(0);

    DynamicImage::ImageRgba8(data_to_img(output).to_rgba())
}

fn black_and_white(img: DynamicImage) -> DynamicImage {
    let white = image::Rgba::<u8>([255, 255, 255, 255]);
    let black = image::Rgba::<u8>([0, 0, 0, 255]);
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
    let white = image::Rgba::<u8>([255, 255, 255, 255]);
    let transparent = image::Rgba::<u8>([0, 0, 0, 0]);

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
