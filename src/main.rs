use clap::Parser;
use image::{imageops, GenericImageView, GrayImage};
use std::path::Path;
use std::time::Instant;

macro_rules! start_timer {
    ($operation_name:expr) => {{
        println!("========== STARTING: {} ==========", $operation_name);
        Instant::now()
    }};
}

macro_rules! stop_timer {
    ($start_time:expr, $operation_name:expr) => {
        let duration = $start_time.elapsed();
        println!(
            "========== COMPLETED: {} in {:.2?} ==========",
            $operation_name, duration
        );
    };
}

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

fn process_image(input: &str) -> Result<(GrayImage, f32), String> {
    let start_time = start_timer!("Image loading");
    let img = image::open(input).map_err(|e| e.to_string())?;
    stop_timer!(start_time, "Image loading");

    let (width, height) = img.dimensions();
    let scaling_factor = 500.0 / height as f32;
    let new_width = (width as f32 * scaling_factor) as u32;

    let start_time = start_timer!("Image resizing");
    let resized_img = img.resize(new_width, 500, imageops::FilterType::Lanczos3);
    stop_timer!(start_time, "Image resizing");

    let start_time = start_timer!("Converting image to grayscale");
    let grayscale_img = resized_img.to_luma8();
    stop_timer!(start_time, "Converting image to grayscale");

    Ok((grayscale_img, scaling_factor))
}

fn detect_edges(_image: &GrayImage) -> Vec<(u32, u32, u32, u32)> {
    let step_time = start_timer!("Applying Gaussian blur");
    let mut blurred_img = imageproc::filter::gaussian_blur_f32(&_image, 2.0);
    stop_timer!(step_time, "Applying Gaussian blur");

    let step_time = start_timer!("Inverting image");
    imageops::colorops::invert(&mut blurred_img);
    stop_timer!(step_time, "Inverting image");

    let step_time = start_timer!("Find contours");
    // use a low threshhold to take advantage of the fact the background should be white.
    let contours: Vec<imageproc::contours::Contour<u32>> =
        imageproc::contours::find_contours_with_threshold(&blurred_img, 10);
    stop_timer!(step_time, "Find contours");
    println!("Found {} contours", contours.len());
    for contour in contours {
        println!("Contour with {} points", contour.points.len());
    }

    // next step: create bounding boxes from contours that are large enough to be interesting
    vec![]
}

// fn crop_images(
//     image: &GrayImage,
//     bounding_boxes: &[(u32, u32, u32, u32)],
//     scaling_factor: f32,
// ) -> Vec<DynamicImage> {
//     bounding_boxes
//         .iter()
//         .map(|&(x, y, w, h)| {
//             let scaled_x = (x as f32 / scaling_factor) as u32;
//             let scaled_y = (y as f32 / scaling_factor) as u32;
//             let scaled_w = (w as f32 / scaling_factor) as u32;
//             let scaled_h = (h as f32 / scaling_factor) as u32;
//             image.crop_images(scaled_x, scaled_y, scaled_w, scaled_h)
//         })
//         .collect()
// }

// fn save_images(images: &[DynamicImage], original_filename: &str) -> Result<(), String> {
//     for (i, img) in images.iter().enumerate() {
//         let output_filename = format!("{}_{}.png", original_filename, i + 1);
//         img.save(&output_filename)
//             .map_err(|_| format!("Failed to save image: {}", output_filename))?;
//     }
//     Ok(())
// }

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

    let (processed_img, scaling_factor) =
        process_image(&args.input).expect("Image processing failed");

    // Get the dimensions of the image
    let (width, height) = processed_img.dimensions();
    println!("==============================");
    println!("Processed image dimensions: {}x{}", width, height);
    println!("Scaling factor: {}", scaling_factor);
    println!("==============================");

    let _bounding_boxes = detect_edges(&processed_img);

    println!("==============================");
    let duration = start_time.elapsed();
    println!("Total time: {:.2?}", duration);
    println!("==============================");
}
