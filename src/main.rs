use clap::{ArgGroup, Parser};
use geo::algorithm::bounding_rect::BoundingRect;
use image::{imageops, GenericImageView, GrayImage};
use std::path::{Path, PathBuf};
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

    /// Optional prefix for filtering files in a folder
    #[arg(long, default_value = "IMG_")]
    prefix: String,

    /// Enable daemon mode to watch a folder for new files
    #[arg(long, action = clap::ArgAction::SetTrue)]
    daemon: bool,
}

fn determine_path_type(input: &str) -> Result<&str, &str> {
    let path = Path::new(input);
    if path.is_dir() {
        Ok("folder")
    } else if path.is_file() {
        Ok("file")
    } else {
        Err("Invalid path.")
    }
}

fn process_image(img: &image::DynamicImage) -> Result<(GrayImage, f64), String> {
    let (width, height) = img.dimensions();
    let scaling_factor = 500.0 / height as f64;
    let new_width = (width as f64 * scaling_factor) as u32;

    let start_time = start_timer!("Image resizing");
    let resized_img = img.resize(new_width, 500, imageops::FilterType::Triangle);
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
        imageproc::contours::find_contours_with_threshold(&blurred_img, 15);
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

fn process_and_crop_image(input_path: &str, _prefix: &str, debug_enabled: bool) -> Vec<String> {
    let img = image::open(input_path).expect("Failed to load image");
    let (processed_img, scaling_factor) = process_image(&img).expect("Image processing failed");

    log_message!(LogLevel::Info, format!("Cropping image: {}", input_path));

    let bounding_boxes = detect_edges(&processed_img);
    let mut output_files = Vec::new();

    for (i, rect) in bounding_boxes.iter().enumerate() {
        let scaled_x = (rect.min().x as f64 / scaling_factor).floor() as u32;
        let scaled_y = (rect.min().y as f64 / scaling_factor).floor() as u32;
        let scaled_w = (rect.width() as f64 / scaling_factor).ceil() as u32;
        let scaled_h = (rect.height() as f64 / scaling_factor).ceil() as u32;

        assert!(scaled_x + scaled_w <= img.width());
        assert!(scaled_y + scaled_h <= img.height());

        let cropped_img = img.crop_imm(scaled_x, scaled_y, scaled_w, scaled_h);
        let output_filename = format!("{}_{}.png", input_path.trim_end_matches(".png"), i + 1);
        cropped_img.save(&output_filename).expect(&format!(
            "Failed to save cropped image: {}",
            output_filename
        ));
        output_files.push(output_filename);
    }

    if debug_enabled {
        let mut debug_img = img.clone().to_rgb8();

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
        }

        debug_img
            .save("debug_with_bounding_boxes.png")
            .expect("Failed to save debug image with bounding boxes");
    }

    log_message!(LogLevel::Info, format!("Completed cropping image: {}", input_path));

    output_files
}

fn main() {
    let args = Cli::parse();
    set_verbosity_level(args.verbose, args.debug);

    let debug_enabled = VERBOSITY_LEVEL.load(Ordering::Relaxed) == LogLevel::Debug as usize;

    log_message!(LogLevel::Info, "Starting the application");

    if args.daemon {
        log_message!(LogLevel::Info, "Daemon mode enabled. Watching folder for new files...");

        let folder_path = Path::new(&args.input);
        if !folder_path.is_dir() {
            log_message!(LogLevel::Error, "Daemon mode requires a valid folder path.");
            return;
        }

        let mut processed_files: std::collections::HashSet<std::path::PathBuf> = std::fs::read_dir(folder_path)
            .expect("Failed to read folder")
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .collect();

        let (tx, rx) = std::sync::mpsc::channel();
        let prefix = args.prefix.clone();
        let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                if let notify::EventKind::Create(_) = event.kind {
                    for path in event.paths {
                        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                            if file_name.starts_with(&prefix) {
                                tx.send(path).expect("Failed to send path through channel");
                            }
                        }
                    }
                }
            }
        }).expect("Failed to create watcher");

        notify::Watcher::watch(&mut watcher, folder_path, notify::RecursiveMode::NonRecursive)
            .expect("Failed to watch folder");

        log_message!(
            LogLevel::Info,
            &format!("Pre-populated processed files with {} existing files.", processed_files.len())
        );

        for path in rx {
            if processed_files.contains(&path) {
                log_message!(LogLevel::Verbose, &format!("File {:?} already processed. Skipping.", path));
                continue;
            }

            log_message!(LogLevel::Info, &format!("New file detected: {:?}", path));

            let mut attempts = 0;
            loop {
                match image::open(&path) {
                    Ok(_) => break,
                    Err(e) if attempts < 10 => {
                        attempts += 1;
                        log_message!(
                            LogLevel::Debug,
                            &format!(
                                "Attempt {} to open file {:?} failed: {}. Retrying...",
                                attempts, path, e
                            )
                        );
                        std::thread::sleep(std::time::Duration::from_millis(2u64.pow(attempts) * 100));
                    }
                    Err(e) => {
                        log_message!(
                            LogLevel::Error,
                            &format!("Failed to open file {:?} after 10 attempts: {}", path, e)
                        );
                        return;
                    }
                }
            }

            let output_files = process_and_crop_image(path.to_str().unwrap(), &args.prefix, debug_enabled);
            processed_files.extend(output_files.into_iter().map(PathBuf::from));
        }

        return;
    }

    let step_time = Instant::now();

    match determine_path_type(&args.input) {
        Ok("file") => {
            log_message!(LogLevel::Info, "File path detected");
            process_and_crop_image(&args.input, &args.prefix, debug_enabled);
        }
        Ok("folder") => {
            log_message!(LogLevel::Info, "Folder path detected");
            for entry in std::fs::read_dir(&args.input).expect("Failed to read folder") {
                let entry = entry.expect("Failed to read entry");
                let path = entry.path();
                if path.is_file() {
                    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                        if file_name.starts_with(&args.prefix) {
                            process_and_crop_image(
                                path.to_str().unwrap(),
                                &args.prefix,
                                debug_enabled,
                            );
                        }
                    }
                }
            }
        }
        Err(e) => {
            log_message!(LogLevel::Error, e);
            return;
        }
        _ => unreachable!(),
    }

    println!("==============================");
    let duration = step_time.elapsed();
    log_message!(LogLevel::Info, &format!("Total time: {:.2?}", duration));
    println!("==============================");
}
