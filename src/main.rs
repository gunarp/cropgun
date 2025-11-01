use clap::{ArgGroup, Parser};
use geo::algorithm::bounding_rect::BoundingRect;
use image::{imageops, GenericImageView, GrayImage};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

macro_rules! start_timer {
    ($operation_name:expr) => {{
        log_message!(
            LogLevel::Verbose,
            &format!("========== STARTING: {} ==========", $operation_name)
        );
        Instant::now()
    }};
}

macro_rules! stop_timer {
    ($start_time:expr, $operation_name:expr) => {
        let duration = $start_time.elapsed();
        log_message!(
            LogLevel::Verbose,
            &format!(
                "========== COMPLETED: {} in {:.2?} ==========",
                $operation_name, duration
            )
        );
    };
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum LogLevel {
    Error,
    Info,
    Verbose,
    Debug,
}

static VERBOSITY_LEVEL: AtomicUsize = AtomicUsize::new(LogLevel::Info as usize);

macro_rules! log_message {
    ($level:expr, $message:expr) => {{
        let current_level = match VERBOSITY_LEVEL.load(Ordering::Relaxed) {
            x if x == LogLevel::Error as usize => LogLevel::Error,
            x if x == LogLevel::Info as usize => LogLevel::Info,
            x if x == LogLevel::Verbose as usize => LogLevel::Verbose,
            x if x == LogLevel::Debug as usize => LogLevel::Debug,
            _ => LogLevel::Info,
        };

        match ($level, current_level) {
            (LogLevel::Error, _) => println!("[ERROR] {}", $message),
            (LogLevel::Info, LogLevel::Info | LogLevel::Verbose | LogLevel::Debug) => {
                println!("[INFO] {}", $message)
            }
            (LogLevel::Verbose, LogLevel::Verbose | LogLevel::Debug) => {
                println!("[VERBOSE] {}", $message)
            }
            (LogLevel::Debug, LogLevel::Debug) => println!("[DEBUG] {}", $message),
            _ => (),
        }
    }};
}

fn set_verbosity_level(verbose: bool, debug: bool) {
    let level = if debug {
        LogLevel::Debug
    } else if verbose {
        LogLevel::Verbose
    } else {
        LogLevel::Info
    };

    VERBOSITY_LEVEL.store(level as usize, Ordering::Relaxed);
}

/// CLI tool to process images
#[derive(Parser)]
#[command(group(ArgGroup::new("verbosity").args(["verbose", "debug"]).multiple(false)))]
struct Cli {
    /// Path to the input PNG file
    input: String,

    /// Enable verbose logging
    #[arg(long, action = clap::ArgAction::SetTrue)]
    verbose: bool,

    /// Enable debug logging (implies verbose)
    #[arg(long, action = clap::ArgAction::SetTrue)]
    debug: bool,
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

fn process_image(img: &image::DynamicImage) -> Result<(GrayImage, f64), String> {
    let (width, height) = img.dimensions();
    let scaling_factor = 500.0 / height as f64;
    let new_width = (width as f64 * scaling_factor) as u32;

    let start_time = start_timer!("Image resizing");
    let resized_img = img.resize(new_width, 500, imageops::FilterType::Lanczos3);
    stop_timer!(start_time, "Image resizing");

    let start_time = start_timer!("Converting image to grayscale");
    let grayscale_img = resized_img.to_luma8();
    stop_timer!(start_time, "Converting image to grayscale");

    Ok((grayscale_img, scaling_factor))
}

fn detect_edges(_image: &GrayImage) -> Vec<geo::Rect<u32>> {
    let step_time = start_timer!("Applying Gaussian blur");
    let mut blurred_img = imageproc::filter::gaussian_blur_f32(&_image, 1.25);
    stop_timer!(step_time, "Applying Gaussian blur");

    let step_time = start_timer!("Inverting image");
    imageops::colorops::invert(&mut blurred_img);
    stop_timer!(step_time, "Inverting image");

    let step_time = start_timer!("Find contours");
    let contours: Vec<imageproc::contours::Contour<u32>> =
        imageproc::contours::find_contours_with_threshold(&blurred_img, 25);
    stop_timer!(step_time, "Find contours");

    log_message!(
        LogLevel::Debug,
        &format!("Found {} contours", contours.len())
    );

    let mut rects: Vec<geo::Rect<u32>> = vec![];
    let image_area = _image.width() * _image.height();
    let min_area = (image_area as f32 * 0.05) as u32;

    for contour in contours {
        log_message!(
            LogLevel::Debug,
            &format!(
                "{} Contour with {} points",
                match contour.border_type {
                    imageproc::contours::BorderType::Hole => "Hole",
                    imageproc::contours::BorderType::Outer => "Outer",
                },
                contour.points.len()
            )
        );

        let points = geo::MultiPoint(
            contour
                .points
                .iter()
                .map(|p| geo::Point::new(p.x, p.y))
                .collect(),
        );

        let bounding_rect = points.bounding_rect().unwrap();
        let rect_area = bounding_rect.width() * bounding_rect.height();

        if rect_area >= min_area {
            rects.push(bounding_rect);

            log_message!(
                LogLevel::Debug,
                &format!(
                    "Bounding box: x={}, y={}, w={}, h={} (area: {})",
                    bounding_rect.min().x,
                    bounding_rect.min().y,
                    bounding_rect.width(),
                    bounding_rect.height(),
                    rect_area
                )
            );
        } else {
            log_message!(
                LogLevel::Debug,
                &format!(
                    "Filtered out bounding box: x={}, y={}, w={}, h={} (area: {})",
                    bounding_rect.min().x,
                    bounding_rect.min().y,
                    bounding_rect.width(),
                    bounding_rect.height(),
                    rect_area
                )
            );
        }
    }

    rects
}

fn main() {
    let args = Cli::parse();
    set_verbosity_level(args.verbose, args.debug);

    let debug_enabled = VERBOSITY_LEVEL.load(Ordering::Relaxed) == LogLevel::Debug as usize;

    log_message!(LogLevel::Info, "Starting the application");

    match determine_path_type(&args.input) {
        Ok("file") => log_message!(LogLevel::Info, "File path detected"),
        Err(e) => {
            log_message!(LogLevel::Error, e);
            return;
        }
        _ => unreachable!(),
    }

    let start_time = Instant::now();

    let img = image::open(&args.input).expect("Failed to load image");
    let (processed_img, scaling_factor) = process_image(&img).expect("Image processing failed");

    log_message!(LogLevel::Info, "Image processing completed");

    // Get the dimensions of the image
    let (width, height) = processed_img.dimensions();
    log_message!(LogLevel::Debug, "==============================");
    log_message!(
        LogLevel::Debug,
        &format!("Processed image dimensions: {}x{}", width, height)
    );
    log_message!(
        LogLevel::Debug,
        &format!("Scaling factor: {}", scaling_factor)
    );
    log_message!(LogLevel::Debug, "==============================");

    let bounding_boxes = detect_edges(&processed_img);

    // Perform cropping operations
    for (i, rect) in bounding_boxes.iter().enumerate() {
        let scaled_x = (rect.min().x as f64 / scaling_factor).floor() as u32;
        let scaled_y = (rect.min().y as f64 / scaling_factor).floor() as u32;
        let scaled_w = (rect.width() as f64 / scaling_factor).ceil() as u32;
        let scaled_h = (rect.height() as f64 / scaling_factor).ceil() as u32;

        // assert that the cropping rectangle is within the image bounds
        assert!(scaled_x + scaled_w <= img.width());
        assert!(scaled_y + scaled_h <= img.height());

        // print cropping rectangle details
        log_message!(
            LogLevel::Debug,
            &format!(
                "Cropping rectangle {}: x={}, y={}, w={}, h={}",
                i + 1,
                scaled_x,
                scaled_y,
                scaled_w,
                scaled_h
            )
        );

        let cropped_img = img.crop_imm(scaled_x, scaled_y, scaled_w, scaled_h);
        let output_filename = format!("{}_{}.png", args.input.trim_end_matches(".png"), i + 1);
        cropped_img.save(&output_filename).expect(&format!(
            "Failed to save cropped image: {}",
            output_filename
        ));
    }

    // Debug-specific logic: Save an image with boundary boxes drawn on the original image
    // Convert the grayscale processed image to RGB format manually
    if debug_enabled {
        let mut debug_img = img.clone().to_rgb8();

        let mut debug_processed_img =
            image::ImageBuffer::from_fn(processed_img.width(), processed_img.height(), |x, y| {
                let pixel = processed_img.get_pixel(x, y);
                image::Rgb([pixel[0], pixel[0], pixel[0]])
            });

        for rect in bounding_boxes.iter() {
            let scaled_x = (rect.min().x as f64 / scaling_factor).round() as u32;
            let scaled_y = (rect.min().y as f64 / scaling_factor).round() as u32;
            let scaled_w = (rect.width() as f64 / scaling_factor).round() as u32;
            let scaled_h = (rect.height() as f64 / scaling_factor).round() as u32;

            imageproc::drawing::draw_hollow_rect_mut(
                &mut debug_img,
                imageproc::rect::Rect::at(scaled_x as i32, scaled_y as i32)
                    .of_size(scaled_w, scaled_h),
                image::Rgb([255, 0, 0]),
            );

            imageproc::drawing::draw_hollow_rect_mut(
                &mut debug_processed_img,
                imageproc::rect::Rect::at(rect.min().x as i32, rect.min().y as i32)
                    .of_size(rect.width(), rect.height()),
                image::Rgb([0, 255, 0]),
            );
        }

        debug_img
            .save("debug_with_bounding_boxes.png")
            .expect("Failed to save debug image with bounding boxes");

        debug_processed_img
            .save("scaled_down_with_boxes.png")
            .expect("Failed to save debug processed image with bounding boxes");
    }

    println!("==============================");
    let duration = start_time.elapsed();
    log_message!(LogLevel::Info, &format!("Total time: {:.2?}", duration));
    println!("==============================");
}
