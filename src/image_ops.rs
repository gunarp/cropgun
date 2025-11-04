use crate::config::{LogLevel, PreprocessingMode};
use crate::{log_message, start_timer, stop_timer};
use geo::algorithm::bounding_rect::BoundingRect;
use image::{imageops, GenericImageView, GrayImage};
use std::borrow::Cow;

const SCALED_IMAGE_HEIGHT: u32 = 1000;

pub fn process_image(img: &image::DynamicImage, preprocess_mode: PreprocessingMode) -> Result<(GrayImage, f64), String> {
    let (scaling_factor, resized_img) = if preprocess_mode == PreprocessingMode::NetworkOptimized {
        let (width, height) = img.dimensions();
        let scaling_factor = (SCALED_IMAGE_HEIGHT as f64) / height as f64;
        let new_width = (width as f64 * scaling_factor) as u32;

        let start_time = start_timer!("Image resizing");
        let resized = img.resize(
            new_width,
            SCALED_IMAGE_HEIGHT,
            imageops::FilterType::Triangle,
        );
        stop_timer!(start_time, "Image resizing");
        (scaling_factor, Cow::Owned(resized))
    } else {
        (1.0, Cow::Borrowed(img))
    };

    let start_time = start_timer!("Converting image to grayscale");
    let grayscale_img = resized_img.to_luma8();
    stop_timer!(start_time, "Converting image to grayscale");

    Ok((grayscale_img, scaling_factor))
}

pub fn detect_edges(_image: &GrayImage) -> Vec<geo::Rect<u32>> {
    let step_time = start_timer!("Applying Gaussian blur");
    let blurred_img = imageproc::filter::gaussian_blur_f32(&_image, 1.25);
    stop_timer!(step_time, "Applying Gaussian blur");

    let step_time = start_timer!("Morphological erosion");
    let eroded = imageproc::morphology::erode(&blurred_img, imageproc::distance_transform::Norm::L2, 1);
    stop_timer!(step_time, "Morphological erosion");

    let step_time = start_timer!("Inverting image");
    let mut inverted_img = eroded.clone();
    imageops::colorops::invert(&mut inverted_img);
    stop_timer!(step_time, "Inverting image");

    let step_time = start_timer!("Find contours");
    let contours: Vec<imageproc::contours::Contour<u32>> =
        imageproc::contours::find_contours_with_threshold(&inverted_img, 15);
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
        
        // Filter by aspect ratio to reject thin lines (like scanner artifacts)
        let width = bounding_rect.width() as f32;
        let height = bounding_rect.height() as f32;
        let aspect_ratio = width.max(height) / width.min(height);

        if rect_area >= min_area && aspect_ratio < 5.0 {
            rects.push(bounding_rect);

            log_message!(
                LogLevel::Debug,
                &format!(
                    "Bounding box: x={}, y={}, w={}, h={} (area: {}, aspect ratio: {:.2})",
                    bounding_rect.min().x,
                    bounding_rect.min().y,
                    width as u32,
                    height as u32,
                    rect_area,
                    aspect_ratio
                )
            );
        } else {
            log_message!(
                LogLevel::Debug,
                &format!(
                    "Filtered out bounding box: x={}, y={}, w={}, h={} (area: {}, aspect ratio: {:.2})",
                    bounding_rect.min().x,
                    bounding_rect.min().y,
                    width as u32,
                    height as u32,
                    rect_area,
                    aspect_ratio
                )
            );
        }
    }

    rects
}
