use clap::Parser;
use image::{DynamicImage, GenericImageView, imageops};
use std::path::Path;
use std::time::Instant;

/// CLI tool to process images
#[derive(Parser)]
struct Cli {
    /// Path to the input PNG file
    input: String,
}

fn determine_path_type(input: &str) -> Result<&str, &str> {
    let path = Path::new(input);
    if path.is_file() {
        Ok("file")
    } else if path.is_dir() {
        Err("Folder paths are not supported yet.")
    } else {
        Err("Invalid path.")
    }
}

fn process_image(input: &str) -> Result<(DynamicImage, f32), String> {
    let start_time = Instant::now();

    let mut img = image::open(input).map_err(|_| "Failed to open image")?;
    let duration = start_time.elapsed();
    println!("Image loading completed in {:.2?}.", duration);

    let step_time = Instant::now();

    // Resize the image to have a height of 500 px
    let (width, height) = img.dimensions();
    let scaling_factor = 500.0 / height as f32;
    let new_width = (width as f32 * scaling_factor) as u32;

    println!("Resizing image to {}x500", new_width);
    img = img.resize(new_width, 500, imageops::FilterType::Lanczos3);
    let duration = step_time.elapsed();
    println!("Image resizing completed in {:.2?}.", duration);

    let step_time = Instant::now();

    // Convert the image to grayscale
    println!("Converting image to grayscale");
    img = img.grayscale();
    let duration = step_time.elapsed();
    println!("Grayscale conversion completed in {:.2?}.", duration);

    let step_time = Instant::now();

    // Apply a Gaussian blur
    println!("Applying Gaussian blur");
    img = DynamicImage::from(imageops::blur(&img, 2.0));
    let duration = step_time.elapsed();
    println!("Gaussian blur completed in {:.2?}.", duration);

    Ok((img, scaling_factor))
}

fn detect_edges(_image: &DynamicImage) -> Vec<(u32, u32, u32, u32)> {
    // Stub for edge detection
    vec![]
}

fn crop_images(image: &DynamicImage, bounding_boxes: &[(u32, u32, u32, u32)], scaling_factor: f32) -> Vec<DynamicImage> {
    bounding_boxes.iter().map(|&(x, y, w, h)| {
        let scaled_x = (x as f32 / scaling_factor) as u32;
        let scaled_y = (y as f32 / scaling_factor) as u32;
        let scaled_w = (w as f32 / scaling_factor) as u32;
        let scaled_h = (h as f32 / scaling_factor) as u32;
        image.crop_imm(scaled_x, scaled_y, scaled_w, scaled_h)
    }).collect()
}

fn save_images(images: &[DynamicImage], original_filename: &str) -> Result<(), String> {
    for (i, img) in images.iter().enumerate() {
        let output_filename = format!("{}_{}.png", original_filename, i + 1);
        img.save(&output_filename).map_err(|_| format!("Failed to save image: {}", output_filename))?;
    }
    Ok(())
}

fn main() {
    let args = Cli::parse();

    match determine_path_type(&args.input) {
        Ok("file") => (),
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
        _ => unreachable!(),
    }

    let start_time = Instant::now();

    let (processed_img, scaling_factor) = process_image(&args.input).expect("Image processing failed");

    let duration = start_time.elapsed();
    println!("Image processing completed in {:.2?}.", duration);

    // Get the dimensions of the image
    let (width, height) = processed_img.dimensions();
    println!("==============================");
    println!("Processed image dimensions: {}x{}", width, height);
    println!("Scaling factor: {}", scaling_factor);

    // // Get the dimensions of the image
    // let (width, height) = img.dimensions();

    // // Calculate the number of pixels
    // let num_pixels = width * height;

    // // Print the number of pixels
    // println!("The image contains {} pixels.", num_pixels);
}